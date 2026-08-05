use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};

use serde_json::Error as JsonError;
use thiserror::Error;

use crate::{TapeEvent, TapeManifest};

const MANIFEST: &str = "manifest.json";
const EVENTS: &str = "events.jsonl";

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
        if root.exists() {
            if !root.is_dir() {
                return Err(TapeStoreError::InvalidRoot {
                    path: root,
                    source: io::Error::new(io::ErrorKind::AlreadyExists, "path is not a directory"),
                });
            }
            return Err(TapeStoreError::AlreadyExists { path: root });
        }
        fs::create_dir_all(&root).map_err(|source| TapeStoreError::InvalidRoot {
            path: root.clone(),
            source,
        })?;
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
        if !store.root.join(EVENTS).is_file() {
            return Err(TapeStoreError::InvalidRoot {
                path: store.root,
                source: io::Error::new(io::ErrorKind::NotFound, "events.jsonl missing"),
            });
        }
        Ok(store)
    }

    pub fn append(&self, event: &TapeEvent) -> Result<(), TapeStoreError> {
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
        let mut file = OpenOptions::new()
            .append(true)
            .open(self.root.join(EVENTS))?;
        serde_json::to_writer(&mut file, event)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        let mut updated = manifest;
        updated.event_count += 1;
        write_manifest(&self.root, &updated)
    }

    pub fn finish(&self, finished_at_ms: u64) -> Result<TapeManifest, TapeStoreError> {
        let mut manifest = self.read_manifest()?;
        if manifest.finished_at_ms.is_some() {
            return Err(TapeStoreError::AlreadyFinished);
        }
        manifest.finished_at_ms = Some(finished_at_ms);
        write_manifest(&self.root, &manifest)?;
        Ok(manifest)
    }

    pub fn read_manifest(&self) -> Result<TapeManifest, TapeStoreError> {
        let bytes = fs::read(self.root.join(MANIFEST))?;
        serde_json::from_slice(&bytes).map_err(|e| TapeStoreError::InvalidManifest(e.to_string()))
    }

    pub fn read_events(
        &self,
    ) -> Result<impl Iterator<Item = Result<TapeEvent, TapeStoreError>>, TapeStoreError> {
        let file = File::open(self.root.join(EVENTS))?;
        let manifest = self.read_manifest()?;
        Ok(EventIter {
            lines: BufReader::new(file).lines(),
            next_sequence: 0,
            line: 0,
            manifest_count: manifest.event_count,
            done: false,
        })
    }
}

struct EventIter<R = io::Lines<BufReader<File>>> {
    lines: R,
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
        let line = match self.lines.next() {
            None if self.next_sequence < self.manifest_count => {
                self.done = true;
                return Some(Err(TapeStoreError::EventCountShortfall {
                    manifest_count: self.manifest_count,
                    event_count: self.next_sequence,
                }));
            }
            None => {
                self.done = true;
                return None;
            }
            Some(line) => line,
        };
        let line = match line {
            Ok(line) => line,
            Err(e) => return Some(Err(e.into())),
        };
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
    let file = File::open(root.join(EVENTS))?;
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
