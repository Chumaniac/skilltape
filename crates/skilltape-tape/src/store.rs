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
        File::create(root.join(EVENTS))?.sync_all()?;
        write_manifest(&root, &manifest)?;
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
        let manifest = self.read_manifest()?;
        if manifest.finished_at_ms.is_some() {
            return Err(TapeStoreError::AlreadyFinished);
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
        self.read_manifest()?;
        Ok(EventIter {
            lines: BufReader::new(file).lines(),
            next_sequence: 0,
            line: 0,
        })
    }
}

struct EventIter<R = io::Lines<BufReader<File>>> {
    lines: R,
    next_sequence: u64,
    line: usize,
}

impl Iterator for EventIter {
    type Item = Result<TapeEvent, TapeStoreError>;
    fn next(&mut self) -> Option<Self::Item> {
        let line = match self.lines.next()? {
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

fn write_manifest(root: &Path, manifest: &TapeManifest) -> Result<(), TapeStoreError> {
    validate_manifest(manifest)?;
    let tmp = root.join("manifest.json.tmp");
    let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    serde_json::to_writer_pretty(&mut file, manifest)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::rename(&tmp, root.join(MANIFEST))?;
    File::open(root)?.sync_all()?;
    Ok(())
}
