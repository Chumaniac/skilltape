//! Deterministic verification and redacted Receipt generation.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::to_vec;
use sha2::{Digest, Sha256};
use skilltape_core::LoadedSkillPackage;
use skilltape_runner::{run_skill, ResourceLimits, RunError, RunEvent, RunRequest};
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

    let skill_hash = digest_tree(&request.package.root)?;
    let input_hash = digest_tree(&request.input_root)?;
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
    for (relative, contents) in files {
        hasher.update(relative.as_bytes());
        hasher.update([0]);
        hasher.update((contents.len() as u64).to_be_bytes());
        hasher.update(contents);
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
    files: &mut Vec<(String, Vec<u8>)>,
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
            files.push((relative, fs::read(&path)?));
        }
    }
    Ok(())
}
