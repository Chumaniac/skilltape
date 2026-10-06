use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};
use skilltape_runner::{publish_directory_noreplace, validate_output_root};
use skilltape_verify::Receipt;
use tempfile::{Builder, TempDir};
use thiserror::Error;

const MAX_ENTRIES: usize = 10_000;
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_METADATA_BYTES: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub(super) enum DeliveryError {
    #[error("destination must be a new safe directory outside the input and package")]
    InvalidDestination,
    #[error("destination already exists")]
    Exists,
    #[error("assembly failed: {0}")]
    Assembly(&'static str),
    #[error("local I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("{reason}; completed artifacts retained in private sibling directory `{directory}`; delivery not published")]
    Recoverable { directory: String, reason: String },
}

impl DeliveryError {
    pub(super) fn is_input(&self) -> bool {
        matches!(self, Self::InvalidDestination | Self::Exists)
    }
}

#[derive(Serialize)]
struct Artifact {
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Serialize)]
struct Manifest<'a> {
    schema: &'static str,
    run_id: &'a str,
    skill_hash: &'a str,
    receipt_sha256: String,
    artifact_set_sha256: String,
    files: Vec<Artifact>,
}

pub(super) struct Delivery {
    target: PathBuf,
    stage: TempDir,
}

impl Delivery {
    pub(super) fn prepare(
        target: &Path,
        package: &Path,
        input: &Path,
    ) -> Result<Self, DeliveryError> {
        if target
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(DeliveryError::InvalidDestination);
        }
        let target = super::absolute_path(target)?;
        if !super::ancestors_are_safe(&target) {
            return Err(DeliveryError::InvalidDestination);
        }
        validate_output_root(package, input, &target)
            .map_err(|_| DeliveryError::InvalidDestination)?;
        let parent = prepare_parent(target.parent().ok_or(DeliveryError::InvalidDestination)?)?;
        let target = parent.join(
            target
                .file_name()
                .ok_or(DeliveryError::InvalidDestination)?,
        );
        if !target.starts_with(&parent) || !super::ancestors_are_safe(&target) {
            return Err(DeliveryError::InvalidDestination);
        }
        validate_output_root(package, input, &target)
            .map_err(|_| DeliveryError::InvalidDestination)?;
        match fs::symlink_metadata(&target) {
            Ok(_) => return Err(DeliveryError::Exists),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let stage = Builder::new()
            .prefix(".skilltape-delivery-")
            .tempdir_in(&parent)?;
        Ok(Self { target, stage })
    }

    pub(super) fn artifact_root(&self) -> PathBuf {
        self.stage.path().join("artifacts")
    }

    pub(super) fn finish(self, receipt: &Receipt) -> Result<(), DeliveryError> {
        let result = self.assemble(receipt).and_then(|()| {
            if !super::ancestors_are_safe(&self.target) {
                return Err(DeliveryError::InvalidDestination);
            }
            publish_directory_noreplace(self.stage.path(), &self.target)
                .map_err(DeliveryError::from)
        });
        if let Err(error) = result {
            let retained = self.stage.keep();
            return Err(DeliveryError::Recoverable {
                reason: error.to_string(),
                directory: retained
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            });
        }
        Ok(())
    }

    fn assemble(&self, receipt: &Receipt) -> Result<(), DeliveryError> {
        let document =
            serde_json::to_vec(receipt).map_err(|_| DeliveryError::Assembly("invalid Receipt"))?;
        if document.len() > MAX_METADATA_BYTES {
            return Err(DeliveryError::Assembly("Receipt exceeds metadata limit"));
        }
        // Preserve the successful run evidence even if artifact assembly fails.
        write_synced(&self.stage.path().join("receipt.json"), &document)?;
        let root = self.artifact_root();
        if !root.exists() {
            fs::create_dir(&root)?;
        }
        let canonical_root = root.canonicalize()?;
        if canonical_root != root || !canonical_root.starts_with(self.stage.path()) {
            return Err(DeliveryError::Assembly("artifact root escaped staging"));
        }
        let mut files = Vec::new();
        let mut entries = 0;
        let mut total = 0;
        let mut metadata_budget = 1024;
        collect(
            &canonical_root,
            &canonical_root,
            0,
            &mut entries,
            &mut total,
            &mut metadata_budget,
            &mut files,
        )?;
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let mut digest = Sha256::new();
        for file in &files {
            digest.update(file.path.as_bytes());
            digest.update([0]);
            digest.update(file.bytes.to_be_bytes());
            digest.update(file.sha256.as_bytes());
        }
        let manifest = Manifest {
            schema: "skilltape.dev/delivery/v1",
            run_id: &receipt.run_id,
            skill_hash: &receipt.skill_hash,
            receipt_sha256: hex(Sha256::digest(&document)),
            artifact_set_sha256: hex(digest.finalize()),
            files,
        };
        let metadata = serde_json::to_vec_pretty(&manifest)
            .map_err(|_| DeliveryError::Assembly("invalid manifest"))?;
        if document.len() > MAX_METADATA_BYTES || metadata.len() > MAX_METADATA_BYTES {
            return Err(DeliveryError::Assembly("metadata exceeds delivery limit"));
        }
        write_synced(&self.stage.path().join("delivery.json"), &metadata)?;
        Ok(())
    }
}

fn prepare_parent(parent: &Path) -> Result<PathBuf, DeliveryError> {
    let mut ancestor = parent;
    let mut missing = Vec::new();
    let mut resolved = loop {
        match ancestor.canonicalize() {
            Ok(path) => break path,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                missing.push(
                    ancestor
                        .file_name()
                        .ok_or(DeliveryError::InvalidDestination)?,
                );
                ancestor = ancestor.parent().ok_or(DeliveryError::InvalidDestination)?;
            }
            Err(error) => return Err(error.into()),
        }
    };
    for name in missing.into_iter().rev() {
        let child = resolved.join(name);
        if !child.starts_with(&resolved) || !super::ancestors_are_safe(&child) {
            return Err(DeliveryError::InvalidDestination);
        }
        match fs::create_dir(&child) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        let canonical = child.canonicalize()?;
        if canonical != child || !canonical.starts_with(&resolved) {
            return Err(DeliveryError::InvalidDestination);
        }
        resolved = canonical;
    }
    Ok(resolved)
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let mut file = File::create_new(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn collect(
    root: &Path,
    current: &Path,
    depth: usize,
    entries: &mut usize,
    total: &mut u64,
    metadata_budget: &mut usize,
    files: &mut Vec<Artifact>,
) -> Result<(), DeliveryError> {
    if depth > 64 {
        return Err(DeliveryError::Assembly("directory depth exceeds limit"));
    }
    let resolved = current.canonicalize()?;
    if resolved != current || !resolved.starts_with(root) {
        return Err(DeliveryError::Assembly(
            "path escaped artifacts or is a symlink",
        ));
    }
    let metadata = fs::symlink_metadata(&resolved)?;
    if metadata.is_symlink() {
        return Err(DeliveryError::Assembly("symlink artifact rejected"));
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(&resolved)? {
            *entries += 1;
            if *entries > MAX_ENTRIES {
                return Err(DeliveryError::Assembly("entry limit exceeded"));
            }
            collect(
                root,
                &entry?.path(),
                depth + 1,
                entries,
                total,
                metadata_budget,
                files,
            )?;
        }
    } else if metadata.is_file() {
        if metadata.len() > MAX_FILE_BYTES || *total + metadata.len() > MAX_TOTAL_BYTES {
            return Err(DeliveryError::Assembly("artifact size exceeds limit"));
        }
        *total += metadata.len();
        let mut parts = Vec::new();
        for part in resolved
            .strip_prefix(root)
            .map_err(|_| DeliveryError::Assembly("path escaped artifacts"))?
            .components()
        {
            let Component::Normal(part) = part else {
                return Err(DeliveryError::Assembly("invalid artifact path"));
            };
            let part = part
                .to_str()
                .ok_or(DeliveryError::Assembly("non-UTF-8 artifact path"))?;
            if part.contains('\\') || part.chars().any(char::is_control) {
                return Err(DeliveryError::Assembly("unsafe artifact path"));
            }
            parts.push(part);
        }
        let path = parts.join("/");
        if path.len() > 1024 {
            return Err(DeliveryError::Assembly("artifact path exceeds limit"));
        }
        // Bound inventory and pretty JSON before allocation, including worst-case escaping.
        *metadata_budget += path.len() * 6 + 256;
        if *metadata_budget > MAX_METADATA_BYTES {
            return Err(DeliveryError::Assembly("manifest budget exceeded"));
        }
        let mut options = File::options();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let mut file = options.open(&resolved)?;
        let opened = file.metadata()?;
        if !opened.is_file() || opened.len() != metadata.len() {
            return Err(DeliveryError::Assembly("artifact changed before assembly"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if opened.dev() != metadata.dev() || opened.ino() != metadata.ino() {
                return Err(DeliveryError::Assembly("artifact replaced before assembly"));
            }
        }
        let mut digest = Sha256::new();
        let mut buffer = [0; 8 * 1024];
        let mut count = 0;
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            count += read as u64;
            if count > metadata.len() {
                return Err(DeliveryError::Assembly("artifact changed during assembly"));
            }
            digest.update(&buffer[..read]);
        }
        if count != metadata.len() || file.metadata()?.len() != metadata.len() {
            return Err(DeliveryError::Assembly("artifact changed during assembly"));
        }
        files.push(Artifact {
            path,
            bytes: count,
            sha256: hex(digest.finalize()),
        });
    } else {
        return Err(DeliveryError::Assembly("special artifact rejected"));
    }
    Ok(())
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;
    use skilltape_verify::ReceiptStatus;

    fn completed_receipt() -> Receipt {
        Receipt {
            schema: "skilltape.dev/receipt/v1".into(),
            run_id: "a".repeat(64),
            skill_hash: "b".repeat(64),
            status: ReceiptStatus::Succeeded,
            steps: Vec::new(),
            assertions: Vec::new(),
            policy_decisions: Vec::new(),
        }
    }

    #[test]
    fn artifact_inventory_rejects_an_empty_directory_outside_its_root() {
        let temp = tempfile::tempdir().expect("temporary inventory");
        let root = temp.path().join("artifacts");
        let outside = temp.path().join("outside");
        fs::create_dir(&root).expect("root");
        fs::create_dir(&outside).expect("outside");
        let mut entries = 0;
        let mut total = 0;
        let mut budget = 1024;
        let mut files = Vec::new();
        assert!(collect(
            &root,
            &outside,
            0,
            &mut entries,
            &mut total,
            &mut budget,
            &mut files,
        )
        .is_err());
    }

    #[test]
    fn successful_material_is_preserved_when_publication_loses_a_race() {
        let temp = tempfile::tempdir().expect("temporary delivery");
        let input = temp.path().join("input");
        let package = temp.path().join("package");
        fs::create_dir(&input).expect("input");
        fs::create_dir(&package).expect("package");
        let target = temp.path().join("published");
        let delivery = Delivery::prepare(&target, &package, &input).expect("prepare");
        fs::create_dir(delivery.artifact_root()).expect("artifacts");
        fs::write(delivery.artifact_root().join("data.txt"), "completed")
            .expect("completed output");
        fs::create_dir(&target).expect("another caller");
        let DeliveryError::Recoverable { directory, .. } = delivery
            .finish(&completed_receipt())
            .expect_err("exclusive publication")
        else {
            panic!("recoverable publication failure");
        };
        assert_eq!(fs::read_dir(&target).expect("existing target").count(), 0);
        assert_eq!(
            fs::read(temp.path().join(directory).join("artifacts/data.txt"))
                .expect("retained output"),
            b"completed"
        );
    }

    #[test]
    fn an_oversized_completed_file_is_retained_without_publishing() {
        let temp = tempfile::tempdir().expect("temporary delivery");
        let input = temp.path().join("input");
        let package = temp.path().join("package");
        fs::create_dir(&input).expect("input");
        fs::create_dir(&package).expect("package");
        let target = temp.path().join("published");
        let delivery = Delivery::prepare(&target, &package, &input).expect("prepare");
        fs::create_dir(delivery.artifact_root()).expect("artifacts");
        File::create(delivery.artifact_root().join("large.bin"))
            .expect("sparse file")
            .set_len(MAX_FILE_BYTES + 1)
            .expect("size");
        let DeliveryError::Recoverable { directory, .. } = delivery
            .finish(&completed_receipt())
            .expect_err("size limit")
        else {
            panic!("recoverable assembly failure");
        };
        assert!(!target.exists());
        assert!(temp.path().join(&directory).join("receipt.json").is_file());
        assert_eq!(
            fs::metadata(temp.path().join(directory).join("artifacts/large.bin"))
                .expect("retained file")
                .len(),
            MAX_FILE_BYTES + 1
        );
    }
}
