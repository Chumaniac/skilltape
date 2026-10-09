//! Deterministic verification and redacted Receipt generation.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde_json::to_vec;
use sha2::{Digest, Sha256};
use skilltape_core::LoadedSkillPackage;
use skilltape_runner::{
    digest_input_tree, preflight_input_capacity, run_skill, ResourceLimits, RunError, RunEvent,
    RunRequest,
};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

mod assertions;
mod receipt;

pub use assertions::{Assertion, AssertionResult};
pub use receipt::{PolicyDecisionSummary, Receipt, ReceiptStatus, ReceiptStep};

pub struct VerifyRequest {
    pub package: LoadedSkillPackage,
    pub input_root: PathBuf,
    pub output_root: PathBuf,
    pub limits: ResourceLimits,
    pub assertions: Vec<Assertion>,
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("verification input root is invalid")]
    InvalidInputRoot,
    #[error("verification assertion is invalid: {message}")]
    InvalidAssertion { message: String },
    #[error("verification assertion failed to read input: {message}")]
    AssertionInput { message: String },
    #[error("runner failed: {0}")]
    Runner(#[from] RunError),
    #[error("verification serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("verification input scan failed: {0}")]
    Io(#[from] io::Error),
    #[error("verification receipt schema failed: {message}")]
    ReceiptSchema { message: String },
}

pub async fn verify_run(request: VerifyRequest) -> Result<Receipt, VerifyError> {
    for assertion in &request.assertions {
        assertion
            .validate()
            .map_err(|error| VerifyError::InvalidAssertion {
                message: error.to_string(),
            })?;
    }
    ensure_directory(&request.input_root).map_err(|_| VerifyError::InvalidInputRoot)?;
    preflight_input_capacity(&request.input_root)?;

    let skill_hash = digest_tree(&request.package.root)?;
    let input_hash = digest_input_tree(&request.input_root)?;
    let assertion_bytes = to_vec(&request.assertions)?;
    let run_id = digest_parts(&[
        skill_hash.as_bytes(),
        input_hash.as_bytes(),
        &assertion_bytes,
    ]);

    let (sender, mut receiver) = mpsc::channel::<RunEvent>(64);
    let run = tokio::spawn(run_skill(
        RunRequest {
            package: request.package,
            input_root: request.input_root.clone(),
            output_root: request.output_root.clone(),
            limits: request.limits,
        },
        skilltape_policy::PolicyEngine::default(),
        sender,
        CancellationToken::new(),
    ));
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    let summary = run.await.map_err(|error| {
        VerifyError::Runner(RunError::Workspace {
            message: format!("runner task failed: {error}"),
        })
    })??;
    for event in &events {
        receipt::validate_run_event(event).map_err(|error| VerifyError::ReceiptSchema {
            message: error.to_string(),
        })?;
    }

    let assertion_results =
        assertions::evaluate(&request.assertions, &request.output_root, &summary).map_err(
            |error| VerifyError::AssertionInput {
                message: error.to_string(),
            },
        )?;
    let receipt = receipt::build(run_id, skill_hash, &summary, assertion_results);
    receipt::validate_receipt(&receipt).map_err(|error| VerifyError::ReceiptSchema {
        message: error.to_string(),
    })?;
    Ok(receipt)
}

fn ensure_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a directory",
        ));
    }
    Ok(())
}

fn digest_tree(root: &Path) -> io::Result<String> {
    ensure_directory(root)?;
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = Sha256::new();
    for (relative, path) in files {
        ensure_hash_file_path(root, &path)?;
        let mut file = fs::File::open(&path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular hash input",
            ));
        }
        hasher.update(relative.as_bytes());
        hasher.update([0]);
        hasher.update(metadata.len().to_be_bytes());
        let mut buffer = [0_u8; 8192];
        let mut total = 0_u64;
        loop {
            let bytes_read = match file.read(&mut buffer) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => result?,
            };
            if bytes_read == 0 {
                break;
            }
            total += bytes_read as u64;
            hasher.update(&buffer[..bytes_read]);
        }
        if total != metadata.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "hash input size changed",
            ));
        }
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>())
}

fn digest_parts(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"skilltape.verify.v1");
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn collect_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> io::Result<()> {
    let mut entries = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "symlink in hashed tree",
            ));
        }
        if metadata.is_dir() {
            collect_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path outside root"))?
                .to_string_lossy()
                .replace('\\', "/");
            files.push((relative, path));
        }
    }
    Ok(())
}

fn ensure_hash_file_path(root: &Path, path: &Path) -> io::Result<()> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "hash path outside root"))?;
    ensure_directory(root)?;
    let mut current = root.to_path_buf();
    for component in relative {
        current.push(component);
        if fs::symlink_metadata(&current)?.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "symlink in hashed tree",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tree_digest_tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn large_binary_tree_keeps_the_legacy_digest_and_global_path_order() {
        let temp = tempdir().expect("temporary tree");
        let root = temp.path();
        fs::create_dir(root.join("a")).expect("nested directory");
        fs::write(root.join("a.txt"), b"first\n").expect("first file");
        fs::write(root.join("a/second.txt"), b"second\n").expect("nested file");
        let mut file = fs::File::create(root.join("large.bin")).expect("binary fixture");
        for _ in 0..256 {
            file.write_all(&[b'A'; 8192]).expect("fixture chunk");
        }
        file.write_all(&[b'A'; 17]).expect("partial final chunk");
        drop(file);
        assert_eq!(
            digest_tree(root).expect("tree digest"),
            "81956d7feb1aadf66e9e6405958c27581381b8851c27d4217cab9ba817584f6b"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_parent_replaced_with_a_symlink_after_inventory() {
        let temp = tempdir().expect("temporary tree");
        let root = temp.path().join("root");
        let outside = temp.path().join("outside");
        fs::create_dir_all(root.join("nested")).expect("root");
        fs::create_dir(&outside).expect("outside");
        fs::write(root.join("nested/data.txt"), b"inside").expect("inside fixture");
        fs::write(outside.join("data.txt"), b"synthetic outside").expect("outside fixture");
        let mut files = Vec::new();
        collect_files(&root, &root, &mut files).expect("inventory");
        fs::rename(root.join("nested"), root.join("original")).expect("move parent");
        std::os::unix::fs::symlink(&outside, root.join("nested")).expect("replace parent");
        assert_eq!(
            ensure_hash_file_path(&root, &files[0].1)
                .expect_err("must not follow the replacement")
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
}
