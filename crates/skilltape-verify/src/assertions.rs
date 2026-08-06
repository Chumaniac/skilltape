use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use skilltape_runner::{RunSummary, StepStatus};
use thiserror::Error;

const MAX_TEXT_ASSERTION_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Assertion {
    FileExists { path: String },
    FileHash { path: String, sha256: String },
    FileTextContains { path: String, text: String },
    CommandExit { step_id: String, code: i32 },
}

impl Assertion {
    pub(crate) fn validate(&self) -> Result<(), AssertionError> {
        match self {
            Self::FileExists { path }
            | Self::FileHash { path, .. }
            | Self::FileTextContains { path, .. } => {
                validate_relative_path(path)?;
            }
            Self::CommandExit { step_id, .. } if step_id.trim().is_empty() => {
                return Err(AssertionError::InvalidTarget);
            }
            Self::CommandExit { .. } => {}
        }
        Ok(())
    }

    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::FileExists { .. } => "file_exists",
            Self::FileHash { .. } => "file_hash",
            Self::FileTextContains { .. } => "file_text_contains",
            Self::CommandExit { .. } => "command_exit",
        }
    }

    pub(crate) fn target(&self) -> &str {
        match self {
            Self::FileExists { path }
            | Self::FileHash { path, .. }
            | Self::FileTextContains { path, .. } => path,
            Self::CommandExit { step_id, .. } => step_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AssertionResult {
    pub kind: String,
    pub target: String,
    pub passed: bool,
    pub reason: String,
}

#[derive(Debug, Error)]
pub(crate) enum AssertionError {
    #[error("assertion target is invalid")]
    InvalidTarget,
    #[error("assertion path is unsafe")]
    UnsafePath,
    #[error("assertion text input is too large")]
    TextTooLarge,
    #[error("assertion input failed: {0}")]
    Io(#[from] io::Error),
}

pub(crate) fn evaluate(
    assertions: &[Assertion],
    output_root: &Path,
    summary: &RunSummary,
) -> Result<Vec<AssertionResult>, AssertionError> {
    assertions
        .iter()
        .map(|assertion| evaluate_one(assertion, output_root, summary))
        .collect()
}

fn evaluate_one(
    assertion: &Assertion,
    output_root: &Path,
    summary: &RunSummary,
) -> Result<AssertionResult, AssertionError> {
    let (passed, reason) = match assertion {
        Assertion::FileExists { path } => {
            let target = resolve_output(output_root, path)?;
            match safe_metadata(&target)? {
                Some(metadata) => (metadata.is_file(), "file exists"),
                None => (false, "file is missing"),
            }
        }
        Assertion::FileHash { path, sha256 } => {
            let target = resolve_output(output_root, path)?;
            let Some(metadata) = safe_metadata(&target)? else {
                return Ok(result(assertion, false, "file is missing"));
            };
            if !metadata.is_file() {
                (false, "target is not a file")
            } else {
                let mut file = fs::File::open(&target)?;
                let mut hasher = Sha256::new();
                let mut buffer = [0_u8; 8192];
                loop {
                    let bytes_read = file.read(&mut buffer)?;
                    if bytes_read == 0 {
                        break;
                    }
                    hasher.update(&buffer[..bytes_read]);
                }
                let actual = format!("{:x}", hasher.finalize());
                (actual.eq_ignore_ascii_case(sha256), "file hash matches")
            }
        }
        Assertion::FileTextContains { path, text } => {
            let target = resolve_output(output_root, path)?;
            let Some(metadata) = safe_metadata(&target)? else {
                return Ok(result(assertion, false, "file is missing"));
            };
            if !metadata.is_file() {
                (false, "target is not a file")
            } else {
                let contents = read_bounded(&target)?;
                (
                    String::from_utf8_lossy(&contents).contains(text),
                    "text assertion evaluated",
                )
            }
        }
        Assertion::CommandExit { step_id, code } => {
            let matching = summary.steps.iter().find(|step| step.step_id == *step_id);
            match matching {
                Some(step) => (
                    step.status != StepStatus::Denied
                        && step.status != StepStatus::SpawnFailed
                        && step.exit_code == Some(*code),
                    "command exit assertion evaluated",
                ),
                None => (false, "step is missing"),
            }
        }
    };

    Ok(result(assertion, passed, reason))
}

fn result(assertion: &Assertion, passed: bool, reason: &str) -> AssertionResult {
    AssertionResult {
        kind: assertion.kind().to_owned(),
        target: assertion.target().to_owned(),
        passed,
        reason: reason.to_owned(),
    }
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, AssertionError> {
    let file = fs::File::open(path)?;
    let mut bounded = file.take(MAX_TEXT_ASSERTION_BYTES + 1);
    let mut contents = Vec::new();
    bounded.read_to_end(&mut contents)?;
    if contents.len() as u64 > MAX_TEXT_ASSERTION_BYTES {
        return Err(AssertionError::TextTooLarge);
    }
    Ok(contents)
}

fn resolve_output(root: &Path, relative: &str) -> Result<PathBuf, AssertionError> {
    let relative = relative.strip_prefix("outputs/").unwrap_or(relative);
    validate_relative_path(relative)?;
    let root = canonical_root(root)?;
    let target = root.join(relative);
    if !target.starts_with(&root) {
        return Err(AssertionError::UnsafePath);
    }
    let mut current = root.clone();
    for component in Path::new(relative).components() {
        let Component::Normal(name) = component else {
            return Err(AssertionError::UnsafePath);
        };
        current.push(name);
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() {
                return Err(AssertionError::UnsafePath);
            }
        }
    }
    Ok(target)
}

fn canonical_root(root: &Path) -> Result<PathBuf, AssertionError> {
    let root = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(AssertionError::Io)?
            .join(root)
    };
    ensure_safe_ancestors(&root)?;
    let mut missing = Vec::new();
    let mut current = root.clone();
    loop {
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    if !is_allowed_system_alias(&current) {
                        return Err(AssertionError::UnsafePath);
                    }
                } else if !metadata.is_dir() {
                    return Err(AssertionError::UnsafePath);
                }
                let mut canonical = current
                    .canonicalize()
                    .map_err(|_| AssertionError::UnsafePath)?;
                for component in missing.iter().rev() {
                    canonical.push(component);
                }
                return Ok(canonical);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let name = current
                    .file_name()
                    .ok_or(AssertionError::UnsafePath)?
                    .to_os_string();
                missing.push(name);
                let parent = current
                    .parent()
                    .ok_or(AssertionError::UnsafePath)?
                    .to_path_buf();
                if parent == current {
                    return Err(AssertionError::UnsafePath);
                }
                current = parent;
            }
            Err(_) => return Err(AssertionError::UnsafePath),
        }
    }
}

fn ensure_safe_ancestors(path: &Path) -> Result<(), AssertionError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata)
                if metadata.file_type().is_symlink() && !is_allowed_system_alias(&current) =>
            {
                return Err(AssertionError::UnsafePath);
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(_) => return Err(AssertionError::UnsafePath),
        }
    }
    Ok(())
}

fn is_allowed_system_alias(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        if path != Path::new("/etc") && path != Path::new("/tmp") && path != Path::new("/var") {
            return false;
        }
        path.canonicalize().is_ok_and(|canonical| {
            matches!(
                canonical.to_str(),
                Some("/private/etc") | Some("/private/tmp") | Some("/private/var")
            )
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        false
    }
}

fn safe_metadata(path: &Path) -> Result<Option<fs::Metadata>, AssertionError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(AssertionError::UnsafePath),
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn validate_relative_path(path: &str) -> Result<(), AssertionError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path.as_bytes().get(1) == Some(&b':')
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "..")
        || Path::new(path).components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(AssertionError::UnsafePath);
    }
    Ok(())
}
