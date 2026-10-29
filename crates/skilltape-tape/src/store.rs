use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};

use fs2::FileExt;
use serde_json::Error as JsonError;
use thiserror::Error;

use crate::{TapeEvent, TapeManifest};

const MANIFEST: &str = "manifest.json";
const EVENTS: &str = "events.jsonl";
const STORE_LOCK: &str = ".store.lock";
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

#[derive(Debug, Error)]
pub enum TapeStoreError {
    #[error("tape root is unsafe: {path}")]
    UnsafeRoot { path: PathBuf },
    #[error("invalid tape root {path}: {source}")]
    InvalidRoot { path: PathBuf, source: io::Error },
    #[error("tape already exists: {path}")]
    AlreadyExists { path: PathBuf },
    #[error("invalid tape manifest: {0}")]
    InvalidManifest(String),
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("invalid JSONL at line {line}: {source}")]
    InvalidJsonl { line: usize, source: JsonError },
    #[error("JSON serialization error: {0}")]
    Json(#[from] JsonError),
    #[error("sequence mismatch at line {line}: expected {expected}, got {actual}")]
    SequenceMismatch {
        expected: u64,
        actual: u64,
        line: usize,
    },
    #[error(
        "manifest expects {manifest_count} events, but events.jsonl ended after {event_count}"
    )]
    EventCountShortfall {
        manifest_count: u64,
        event_count: u64,
    },
    #[error(
        "manifest expects {manifest_count} events, but events.jsonl contains at least {minimum_event_count}"
    )]
    EventCountExceeded {
        manifest_count: u64,
        minimum_event_count: u64,
    },
    #[error("tape is already finished")]
    AlreadyFinished,
    #[error("tape exceeds the configured event read budget")]
    ReadBudgetExceeded,
}

pub struct TapeStore {
    root: PathBuf,
}

impl TapeStore {
    pub fn create(
        root: impl Into<PathBuf>,
        manifest: TapeManifest,
    ) -> Result<Self, TapeStoreError> {
        let root = root.into();
        validate_root(&root)?;
        validate_manifest(&manifest)?;
        if let Some(parent) = root
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| TapeStoreError::InvalidRoot {
                path: root.clone(),
                source,
            })?;
        }
        match fs::create_dir(&root) {
            Ok(()) => {}
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists && root.is_dir() => {
                return Err(TapeStoreError::AlreadyExists { path: root });
            }
            Err(source) => {
                return Err(TapeStoreError::InvalidRoot { path: root, source });
            }
        }
        if let Err(error) = initialize_tape(&root, &manifest) {
            let _ = fs::remove_dir_all(&root);
            return Err(error);
        }
        Ok(Self { root })
    }

    pub fn open(root: impl Into<PathBuf>) -> Result<Self, TapeStoreError> {
        let root = root.into();
        validate_root(&root)?;
        if !root.is_dir() {
            return Err(TapeStoreError::InvalidRoot {
                path: root,
                source: io::Error::new(io::ErrorKind::NotFound, "not a directory"),
            });
        }
        let store = Self { root };
        store.read_manifest()?;
        open_data_file(&store.root, EVENTS, false)?;
        Ok(store)
    }

    pub fn append(&self, event: &TapeEvent) -> Result<(), TapeStoreError> {
        let _lock = self.lock_exclusive()?;
        let mut manifest = self.read_manifest()?;
        if manifest.finished_at_ms.is_some() {
            return Err(TapeStoreError::AlreadyFinished);
        }
        let events = read_event_file(&self.root)?;
        let event_count = events.len() as u64;
        if event_count < manifest.event_count {
            return Err(TapeStoreError::EventCountShortfall {
                manifest_count: manifest.event_count,
                event_count,
            });
        }
        if event_count > manifest.event_count {
            if event_count != manifest.event_count + 1 {
                return Err(TapeStoreError::EventCountExceeded {
                    manifest_count: manifest.event_count,
                    minimum_event_count: event_count,
                });
            }

            let recovered_event = events.last().expect("event count is non-zero");
            manifest.event_count = event_count;
            write_manifest(&self.root, &manifest)?;
            if recovered_event == event {
                return Ok(());
            }
        }
        let expected = manifest.event_count;
        if event.sequence != expected {
            return Err(TapeStoreError::SequenceMismatch {
                expected,
                actual: event.sequence,
                line: expected as usize + 1,
            });
        }
        let mut file = open_data_file(&self.root, EVENTS, true)?;
        serde_json::to_writer(&mut file, event)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        let mut updated = manifest;
        updated.event_count += 1;
        write_manifest(&self.root, &updated)
    }

    pub fn finish(&self, finished_at_ms: u64) -> Result<TapeManifest, TapeStoreError> {
        let _lock = self.lock_exclusive()?;
        let mut manifest = self.read_manifest()?;
        if manifest.finished_at_ms.is_some() {
            return Err(TapeStoreError::AlreadyFinished);
        }
        let event_count = read_event_file(&self.root)?.len() as u64;
        if event_count < manifest.event_count {
            return Err(TapeStoreError::EventCountShortfall {
                manifest_count: manifest.event_count,
                event_count,
            });
        }
        if event_count > manifest.event_count + 1 {
            return Err(TapeStoreError::EventCountExceeded {
                manifest_count: manifest.event_count,
                minimum_event_count: event_count,
            });
        }
        manifest.event_count = event_count;
        manifest.finished_at_ms = Some(finished_at_ms);
        write_manifest(&self.root, &manifest)?;
        Ok(manifest)
    }

    pub fn read_manifest(&self) -> Result<TapeManifest, TapeStoreError> {
        let file = open_data_file(&self.root, MANIFEST, false)?;
        let mut bytes = Vec::new();
        file.take(MAX_MANIFEST_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_MANIFEST_BYTES {
            return Err(TapeStoreError::InvalidManifest(
                "manifest exceeds 64 KiB read limit".into(),
            ));
        }
        serde_json::from_slice(&bytes).map_err(|e| TapeStoreError::InvalidManifest(e.to_string()))
    }

    pub fn read_events(
        &self,
    ) -> Result<impl Iterator<Item = Result<TapeEvent, TapeStoreError>>, TapeStoreError> {
        self.event_iter(None, None)
    }

    /// Stream a Tape within explicit byte and event budgets, including file growth.
    pub fn read_events_bounded(
        &self,
        max_bytes: u64,
        max_events: u64,
    ) -> Result<impl Iterator<Item = Result<TapeEvent, TapeStoreError>>, TapeStoreError> {
        self.event_iter(Some(max_bytes), Some(max_events))
    }

    fn event_iter(
        &self,
        max_bytes: Option<u64>,
        max_events: Option<u64>,
    ) -> Result<EventIter, TapeStoreError> {
        let file = open_data_file(&self.root, EVENTS, false)?;
        if max_bytes.is_some_and(|limit| file.metadata().map_or(true, |m| m.len() > limit)) {
            return Err(TapeStoreError::ReadBudgetExceeded);
        }
        let manifest = self.read_manifest()?;
        if max_events.is_some_and(|limit| manifest.event_count > limit) {
            return Err(TapeStoreError::ReadBudgetExceeded);
        }
        Ok(EventIter {
            reader: BufReader::new(file),
            remaining_bytes: max_bytes,
            next_sequence: 0,
            line: 0,
            manifest_count: manifest.event_count,
            done: false,
        })
    }

    fn lock_exclusive(&self) -> Result<File, TapeStoreError> {
        let file = protected_options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.root.join(STORE_LOCK))?;
        if !file.metadata()?.is_file() {
            return Err(TapeStoreError::UnsafeRoot {
                path: self.root.clone(),
            });
        }
        FileExt::lock_exclusive(&file)?;
        Ok(file)
    }
}

fn protected_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    options
}

/// The selected canonical Tape directory is the authority. Data children must
/// remain regular files inside it, including after opening the file handle.
fn open_data_file(root: &Path, name: &str, append: bool) -> Result<File, TapeStoreError> {
    let root = root.canonicalize()?;
    let candidate = root.join(name);
    let before = fs::symlink_metadata(&candidate)?;
    if !before.is_file() || before.file_type().is_symlink() {
        return Err(TapeStoreError::UnsafeRoot { path: candidate });
    }
    let path = candidate.canonicalize()?;
    if !path.starts_with(&root) || path.parent() != Some(root.as_path()) {
        return Err(TapeStoreError::UnsafeRoot { path });
    }
    let file = protected_options()
        .read(!append)
        .append(append)
        .open(&path)?;
    let opened = file.metadata()?;
    let current = fs::symlink_metadata(&candidate)?;
    if !opened.is_file()
        || !current.is_file()
        || current.file_type().is_symlink()
        || candidate.canonicalize()? != path
        || !same_data_file(&before, &opened)
        || !same_data_file(&current, &opened)
    {
        return Err(TapeStoreError::UnsafeRoot { path });
    }
    Ok(file)
}

#[cfg(unix)]
fn same_data_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_data_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if left.file_attributes() & 0x400 != 0 || right.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    left.len() == right.len()
        && left
            .modified()
            .ok()
            .zip(right.modified().ok())
            .is_some_and(|(left, right)| left == right)
}

struct EventIter {
    reader: BufReader<File>,
    remaining_bytes: Option<u64>,
    next_sequence: u64,
    line: usize,
    manifest_count: u64,
    done: bool,
}

impl Iterator for EventIter {
    type Item = Result<TapeEvent, TapeStoreError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let mut line = String::new();
        let read = match self.remaining_bytes {
            Some(remaining) => self
                .reader
                .by_ref()
                .take(remaining.saturating_add(1))
                .read_line(&mut line),
            None => self.reader.read_line(&mut line),
        };
        let bytes = match read {
            Ok(bytes) => bytes as u64,
            Err(error) => return Some(Err(error.into())),
        };
        if let Some(remaining) = self.remaining_bytes {
            if bytes > remaining {
                self.done = true;
                return Some(Err(TapeStoreError::ReadBudgetExceeded));
            }
            self.remaining_bytes = Some(remaining - bytes);
        }
        match bytes {
            0 if self.next_sequence < self.manifest_count => {
                self.done = true;
                return Some(Err(TapeStoreError::EventCountShortfall {
                    manifest_count: self.manifest_count,
                    event_count: self.next_sequence,
                }));
            }
            0 => {
                self.done = true;
                return None;
            }
            _ => {}
        }
        self.line += 1;
        let event: TapeEvent = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(source) => {
                return Some(Err(TapeStoreError::InvalidJsonl {
                    line: self.line,
                    source,
                }))
            }
        };
        if event.sequence != self.next_sequence {
            return Some(Err(TapeStoreError::SequenceMismatch {
                expected: self.next_sequence,
                actual: event.sequence,
                line: self.line,
            }));
        }
        self.next_sequence += 1;
        if self.next_sequence > self.manifest_count {
            self.done = true;
            return Some(Err(TapeStoreError::EventCountExceeded {
                manifest_count: self.manifest_count,
                minimum_event_count: self.next_sequence,
            }));
        }
        Some(Ok(event))
    }
}

fn validate_root(root: &Path) -> Result<(), TapeStoreError> {
    if root.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(TapeStoreError::UnsafeRoot {
            path: root.to_path_buf(),
        });
    }
    Ok(())
}

fn validate_manifest(manifest: &TapeManifest) -> Result<(), TapeStoreError> {
    if serde_json::to_vec_pretty(manifest)?.len() as u64 + 1 > MAX_MANIFEST_BYTES {
        return Err(TapeStoreError::InvalidManifest(
            "manifest exceeds 64 KiB read limit".into(),
        ));
    }
    serde_json::to_value(manifest)
        .and_then(serde_json::from_value::<TapeManifest>)
        .map(|_| ())
        .map_err(|e| TapeStoreError::InvalidManifest(e.to_string()))
}

fn initialize_tape(root: &Path, manifest: &TapeManifest) -> Result<(), TapeStoreError> {
    File::create(root.join(EVENTS))?.sync_all()?;
    write_manifest(root, manifest)
}

fn read_event_file(root: &Path) -> Result<Vec<TapeEvent>, TapeStoreError> {
    let file = open_data_file(root, EVENTS, false)?;
    let mut events = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line_number = index + 1;
        let line = line?;
        let event: TapeEvent =
            serde_json::from_str(&line).map_err(|source| TapeStoreError::InvalidJsonl {
                line: line_number,
                source,
            })?;
        let expected = events.len() as u64;
        if event.sequence != expected {
            return Err(TapeStoreError::SequenceMismatch {
                expected,
                actual: event.sequence,
                line: line_number,
            });
        }
        events.push(event);
    }
    Ok(events)
}

fn write_manifest(root: &Path, manifest: &TapeManifest) -> Result<(), TapeStoreError> {
    validate_manifest(manifest)?;
    let tmp = root.join("manifest.json.tmp");
    match fs::remove_file(&tmp) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    serde_json::to_writer_pretty(&mut file, manifest)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::rename(&tmp, root.join(MANIFEST))?;
    File::open(root)?.sync_all()?;
    Ok(())
}
