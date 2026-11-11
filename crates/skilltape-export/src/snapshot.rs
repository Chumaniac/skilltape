//! Bounded selected-file snapshots for exports, without reading unrelated files.
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::generic::ensure_source_ancestors;
use crate::ExportError;

pub(crate) const MAX_ENTRIES: usize = 10_000;
pub(crate) const MAX_DEPTH: usize = 64;
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) struct Snapshot {
    root: PathBuf,
    entries: Vec<(String, Metadata)>,
}

fn changed() -> ExportError {
    ExportError::ChangedSource
}

fn io_at(path: &Path, source: io::Error) -> ExportError {
    ExportError::Io {
        path: path.to_owned(),
        source,
    }
}

fn same_metadata(left: &Metadata, right: &Metadata) -> io::Result<bool> {
    if !left.is_file()
        || !right.is_file()
        || left.len() != right.len()
        || left.modified()? != right.modified()?
        || left.permissions().readonly() != right.permissions().readonly()
    {
        return Ok(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if left.dev() != right.dev()
            || left.ino() != right.ino()
            || left.ctime() != right.ctime()
            || left.ctime_nsec() != right.ctime_nsec()
            || left.mode() != right.mode()
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn open_observed(root: &Path, relative: &str, expected: &Metadata) -> Result<File, ExportError> {
    ensure_source_ancestors(root, relative)?;
    let path = root.join(relative);
    if path.canonicalize().map_err(|e| io_at(&path, e))? != path {
        return Err(changed());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = options.open(&path).map_err(|e| io_at(&path, e))?;
    let metadata = file.metadata().map_err(|e| io_at(&path, e))?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(changed());
        }
    }
    if !same_metadata(expected, &metadata).map_err(|e| io_at(&path, e))? {
        return Err(changed());
    }
    Ok(file)
}

pub(crate) fn read_observed(
    reader: &mut impl Read,
    size: u64,
    mut consume: impl FnMut(&[u8]) -> io::Result<()>,
) -> io::Result<()> {
    let mut remaining = size;
    let mut buffer = [0u8; 8192];
    while remaining > 0 {
        let limit = remaining.min(buffer.len() as u64) as usize;
        let count = match reader.read(&mut buffer[..limit]) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            value => value?,
        };
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "export source changed",
            ));
        }
        consume(&buffer[..count])?;
        remaining -= count as u64;
    }
    loop {
        match reader.read(&mut buffer[..1]) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Ok(0) => return Ok(()),
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "export source grew",
                ))
            }
            Err(error) => return Err(error),
        }
    }
}

impl Snapshot {
    pub(crate) fn capture(root: &Path, files: &[String]) -> Result<Self, ExportError> {
        if files.len() > MAX_ENTRIES {
            return Err(ExportError::Capacity);
        }
        let mut total = 0u64;
        let mut entries = Vec::with_capacity(files.len());
        for relative in files {
            ensure_source_ancestors(root, relative)?;
            let path = root.join(relative);
            let metadata = fs::symlink_metadata(&path).map_err(|e| io_at(&path, e))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(changed());
            }
            total = total
                .checked_add(metadata.len())
                .ok_or(ExportError::Capacity)?;
            if metadata.len() > MAX_FILE_BYTES || total > MAX_TOTAL_BYTES {
                return Err(ExportError::Capacity);
            }
            entries.push((relative.clone(), metadata));
        }
        Ok(Self {
            root: root.to_owned(),
            entries,
        })
    }

    pub(crate) fn copy_to(&self, destination: &Path) -> Result<(), ExportError> {
        for (relative, metadata) in &self.entries {
            let source = self.root.join(relative);
            let mut input = open_observed(&self.root, relative, metadata)?;
            let target = destination.join(relative);
            fs::create_dir_all(target.parent().expect("selected relative file parent"))
                .map_err(|e| io_at(&target, e))?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|e| io_at(&target, e))?;
            read_observed(&mut input, metadata.len(), |bytes| output.write_all(bytes))
                .map_err(|e| io_at(&source, e))?;
            if !same_metadata(metadata, &input.metadata().map_err(|e| io_at(&source, e))?)
                .map_err(|e| io_at(&source, e))?
            {
                return Err(changed());
            }
            fs::set_permissions(&target, metadata.permissions()).map_err(|e| io_at(&target, e))?;
        }
        self.check()
    }

    pub(crate) fn check(&self) -> Result<(), ExportError> {
        for (relative, expected) in &self.entries {
            ensure_source_ancestors(&self.root, relative)?;
            let path = self.root.join(relative);
            let actual = fs::symlink_metadata(&path).map_err(|e| io_at(&path, e))?;
            if actual.file_type().is_symlink()
                || !same_metadata(expected, &actual).map_err(|e| io_at(&path, e))?
            {
                return Err(changed());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[test]
    fn observed_read_rejects_growth_and_truncation_with_a_bounded_probe() {
        let mut count = 0;
        assert!(
            read_observed(&mut Cursor::new(vec![1u8; 100_000]), 8192, |bytes| {
                count += bytes.len();
                Ok(())
            })
            .is_err()
        );
        assert_eq!(count, 8192);
        assert!(read_observed(&mut Cursor::new(b"ab"), 3, |_| Ok(())).is_err());
        assert!(read_observed(&mut Cursor::new(b"abc"), 3, |_| Ok(())).is_ok());
    }
    #[test]
    fn a_source_replaced_after_inventory_is_not_copied() {
        let root = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        fs::write(root.path().join("data"), "old").unwrap();
        let snapshot = Snapshot::capture(root.path(), &["data".into()]).unwrap();
        fs::write(root.path().join("replacement"), "new").unwrap();
        fs::rename(root.path().join("replacement"), root.path().join("data")).unwrap();
        assert!(snapshot.copy_to(destination.path()).is_err());
        assert!(!destination.path().join("data").exists());
    }
}
