//! Bounded package/output I/O reuses the input reader's no-link and growth checks.
use std::collections::BTreeMap;
use std::fs::{self, Metadata, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use skilltape_core::{
    ARTIFACT_MAX_DEPTH, ARTIFACT_MAX_ENTRIES, ARTIFACT_MAX_FILE_BYTES, ARTIFACT_MAX_TOTAL_BYTES,
};

use crate::input_io::{open_input, read_observed, same_metadata};
use crate::workspace::{ensure_no_symlink_ancestors, lexical_workspace_path, WorkspaceError};

#[derive(Default)]
pub(crate) struct ArtifactBudget {
    entries: BTreeMap<PathBuf, u64>,
    bytes: u64,
    previous_entries: u64,
}

impl ArtifactBudget {
    /// File-step copies charge each operation, including a repeated destination
    /// after a move. Only observations within one copy inventory are deduplicated.
    pub(crate) fn begin_copy(&mut self) {
        self.previous_entries += self.entries.len() as u64;
        self.entries.clear();
    }

    /// Accounting is deduplicated, never the reads themselves. A changing path
    /// retains its high-water size so repeated assertions cannot release budget.
    pub(crate) fn observe(
        &mut self,
        path: &Path,
        metadata: &Metadata,
        depth: u64,
    ) -> Result<(), WorkspaceError> {
        limit(depth, ARTIFACT_MAX_DEPTH, "depth")?;
        let key = lexical_workspace_path(path)?;
        let previous = self.entries.get(&key).copied();
        limit(
            self.previous_entries + self.entries.len() as u64 + u64::from(previous.is_none()),
            ARTIFACT_MAX_ENTRIES,
            "entries",
        )?;
        let size = if metadata.is_file() {
            metadata.len()
        } else {
            0
        };
        limit(size, ARTIFACT_MAX_FILE_BYTES, "file_bytes")?;
        let high_water = size.max(previous.unwrap_or(0));
        let total = self.bytes + high_water - previous.unwrap_or(0);
        limit(total, ARTIFACT_MAX_TOTAL_BYTES, "total_bytes")?;
        self.entries.insert(key, high_water);
        self.bytes = total;
        Ok(())
    }
}

fn limit(value: u64, ceiling: u64, dimension: &'static str) -> Result<(), WorkspaceError> {
    if value > ceiling {
        return Err(WorkspaceError::ArtifactCapacity {
            dimension,
            limit: ceiling,
        });
    }
    Ok(())
}

fn io_error(path: &Path, source: io::Error) -> WorkspaceError {
    WorkspaceError::Io {
        path: path.to_owned(),
        source,
    }
}

fn read_source(
    root: &Path,
    path: &Path,
    metadata: &Metadata,
    consume: impl FnMut(&[u8]) -> io::Result<()>,
) -> Result<(), WorkspaceError> {
    ensure_no_symlink_ancestors(root)?;
    let root = root.canonicalize().map_err(|error| io_error(root, error))?;
    let mut source = open_input(&root, path, metadata)?;
    read_observed(&mut source, metadata.len(), consume).map_err(|error| io_error(path, error))?;
    ensure_no_symlink_ancestors(path)?;
    let current = fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if !same_metadata(metadata, &current).map_err(|error| io_error(path, error))?
        || !same_metadata(
            metadata,
            &source.metadata().map_err(|error| io_error(path, error))?,
        )
        .map_err(|error| io_error(path, error))?
    {
        return Err(io_error(
            path,
            io::Error::new(
                io::ErrorKind::InvalidData,
                "source changed during bounded I/O",
            ),
        ));
    }
    Ok(())
}

pub(crate) fn copy_file(
    root: &Path,
    source: &Path,
    destination: &Path,
    metadata: &Metadata,
) -> Result<(), WorkspaceError> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| io_error(destination, error))?;
    read_source(root, source, metadata, |bytes| output.write_all(bytes))
}

pub(crate) fn hash_file(
    root: &Path,
    path: &Path,
    metadata: &Metadata,
) -> Result<String, WorkspaceError> {
    let mut digest = Sha256::new();
    read_source(root, path, metadata, |bytes| {
        digest.update(bytes);
        Ok(())
    })?;
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
