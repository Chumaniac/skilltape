use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use skilltape_runner::{RunSummary, StepStatus};
use thiserror::Error;

const MAX_TEXT_ASSERTION_BYTES: u64 = 1024 * 1024;
const MAX_HASH_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_HASH_TOTAL_BYTES: u64 = 64 * 1024 * 1024;

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
        if let Self::FileHash { sha256, .. } = self {
            if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(AssertionError::InvalidTarget);
            }
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
    #[error("assertion hash input exceeds the 16 MiB file or 64 MiB total limit")]
    HashTooLarge,
    #[error("assertion hash input changed while reading")]
    ChangedInput,
    #[error("assertion input failed: {0}")]
    Io(#[from] io::Error),
}

pub(crate) fn evaluate(
    assertions: &[Assertion],
    output_root: &Path,
    summary: &RunSummary,
) -> Result<Vec<AssertionResult>, AssertionError> {
    let mut hash_budget = MAX_HASH_TOTAL_BYTES;
    assertions
        .iter()
        .map(|assertion| evaluate_one(assertion, output_root, summary, &mut hash_budget))
        .collect()
}

fn evaluate_one(
    assertion: &Assertion,
    output_root: &Path,
    summary: &RunSummary,
    hash_budget: &mut u64,
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
                let actual = hash_output(&target, &metadata, hash_budget)?;
                let matches = actual.eq_ignore_ascii_case(sha256);
                (
                    matches,
                    if matches {
                        "file hash matches"
                    } else {
                        "file hash differs"
                    },
                )
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

fn hash_output(
    path: &Path,
    expected: &fs::Metadata,
    remaining: &mut u64,
) -> Result<String, AssertionError> {
    if expected.len() > MAX_HASH_FILE_BYTES || expected.len() > *remaining {
        return Err(AssertionError::HashTooLarge);
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !same_hash_metadata(expected, &file.metadata()?)? {
        return Err(AssertionError::ChangedInput);
    }
    // The extra byte detects growth without following an unbounded writer.
    let mut reader = file.take(expected.len() + 1);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    let mut total = 0_u64;
    loop {
        let count = match reader.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            break;
        }
        total += count as u64;
        *remaining = remaining
            .checked_sub(count as u64)
            .ok_or(AssertionError::HashTooLarge)?;
        hasher.update(&buffer[..count]);
    }
    let current = safe_metadata(path)?.ok_or(AssertionError::ChangedInput)?;
    if total != expected.len()
        || !same_hash_metadata(expected, &reader.get_ref().metadata()?)?
        || !same_hash_metadata(expected, &current)?
    {
        return Err(AssertionError::ChangedInput);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn same_hash_metadata(left: &fs::Metadata, right: &fs::Metadata) -> io::Result<bool> {
    if !right.is_file()
        || right.file_type().is_symlink()
        || left.len() != right.len()
        || left.modified()? != right.modified()?
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
        {
            return Ok(false);
        }
    }
    Ok(true)
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
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn hash_checks_reject_oversized_files_without_reading_them() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("large");
        fs::File::create(&path)
            .unwrap()
            .set_len(MAX_HASH_FILE_BYTES + 1)
            .unwrap();
        let mut budget = MAX_HASH_TOTAL_BYTES;
        assert!(matches!(
            hash_output(&path, &fs::metadata(&path).unwrap(), &mut budget),
            Err(AssertionError::HashTooLarge)
        ));
        assert_eq!(budget, MAX_HASH_TOTAL_BYTES);
    }

    #[test]
    fn hash_checks_share_a_total_read_budget() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("value");
        fs::write(&path, b"abc").unwrap();
        let metadata = fs::metadata(&path).unwrap();
        let mut budget = 3;
        assert_eq!(
            hash_output(&path, &metadata, &mut budget).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(budget, 0);
        assert!(matches!(
            hash_output(&path, &metadata, &mut budget),
            Err(AssertionError::HashTooLarge)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn hash_checks_reject_a_replaced_file_with_the_same_size() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("value");
        let other = temp.path().join("replacement");
        fs::write(&path, b"abc").unwrap();
        let metadata = fs::metadata(&path).unwrap();
        fs::write(&other, b"xyz").unwrap();
        fs::rename(other, &path).unwrap();
        let mut budget = MAX_HASH_TOTAL_BYTES;
        assert!(matches!(
            hash_output(&path, &metadata, &mut budget),
            Err(AssertionError::ChangedInput)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn replacing_a_hash_input_with_a_fifo_cannot_block_verification() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        use std::time::Duration;
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("value");
        fs::write(&path, b"abc").unwrap();
        let metadata = fs::metadata(&path).unwrap();
        fs::remove_file(&path).unwrap();
        let name = CString::new(path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut budget = MAX_HASH_TOTAL_BYTES;
            let _ = send.send(hash_output(&path, &metadata, &mut budget).is_err());
        });
        assert!(receive.recv_timeout(Duration::from_secs(2)).unwrap());
    }
}
