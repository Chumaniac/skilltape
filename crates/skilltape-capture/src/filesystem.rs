use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use notify::event::{ModifyKind, RenameMode};
use notify::{Config, Event, EventKind, PollWatcher, RecursiveMode, Watcher};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const BATCH_WINDOW: Duration = Duration::from_millis(40);
const DEDUPLICATION_WINDOW: Duration = Duration::from_millis(250);
// A bounded poll interval is reliable on macOS CI runners where FSEvents can
// accept a watch without delivering file-level events for temporary roots.
const WATCH_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// The logical kind of a captured workspace filesystem change.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FilesystemChangeKind {
    /// A path was created inside the workspace.
    Created,
    /// An existing path's content or metadata changed.
    Modified,
    /// A path was renamed or moved within the workspace.
    Moved,
    /// A path was deleted from the workspace.
    Deleted,
}

/// Metadata-only description of one workspace filesystem change.
///
/// Paths are normalized, `/`-separated, and relative to the watched root.
/// File contents are read only to calculate `content_hash` and are not retained.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FilesystemChange {
    /// The logical operation observed for this path.
    pub kind: FilesystemChangeKind,
    /// The normalized workspace-relative destination or affected path.
    pub path: String,
    /// The normalized workspace-relative source path for a move.
    pub previous_path: Option<String>,
    /// Lowercase SHA-256 of current file contents, when the path is a file.
    pub content_hash: Option<String>,
    /// Current file size in bytes, absent when the path no longer exists.
    pub size: Option<u64>,
}

impl Ord for FilesystemChange {
    fn cmp(&self, other: &Self) -> Ordering {
        self.path
            .cmp(&other.path)
            .then_with(|| self.kind.cmp(&other.kind))
            .then_with(|| self.previous_path.cmp(&other.previous_path))
            .then_with(|| self.content_hash.cmp(&other.content_hash))
            .then_with(|| self.size.cmp(&other.size))
    }
}

impl PartialOrd for FilesystemChange {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Typed failures produced while setting up or running workspace capture.
#[derive(Debug, Error)]
pub enum FilesystemCaptureError {
    /// The supplied root is not an existing directory.
    #[error("filesystem capture root is not a directory: {0}")]
    InvalidRoot(PathBuf),
    /// The root could not be resolved to a stable absolute path.
    #[error("failed to resolve filesystem capture root {path}: {source}")]
    ResolveRoot {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A watcher reported a path that resolves outside the workspace root.
    #[error("filesystem event path escapes workspace root: {0}")]
    PathOutsideRoot(PathBuf),
    /// File metadata or hashing failed.
    #[error("failed to inspect captured path {path}: {source}")]
    InspectPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The platform watcher failed.
    #[error("filesystem watcher error: {0}")]
    Watcher(#[from] notify::Error),
    /// The capture consumer closed before cancellation.
    #[error("filesystem change receiver closed")]
    ReceiverClosed,
}

/// Watches `root` recursively and sends normalized metadata-only changes.
///
/// Events are de-duplicated and emitted in deterministic path order within a
/// short batch. Cancellation drops the platform watcher before returning.
pub async fn watch_workspace(
    root: &Path,
    tx: mpsc::Sender<FilesystemChange>,
    cancel: CancellationToken,
) -> Result<(), FilesystemCaptureError> {
    if !root.is_dir() {
        return Err(FilesystemCaptureError::InvalidRoot(root.to_owned()));
    }
    let canonical_root =
        root.canonicalize()
            .map_err(|source| FilesystemCaptureError::ResolveRoot {
                path: root.to_owned(),
                source,
            })?;
    let (raw_tx, mut raw_rx) = mpsc::unbounded_channel();
    let mut watcher = PollWatcher::new(
        move |event| {
            let _ = raw_tx.send(event);
        },
        Config::default()
            .with_poll_interval(WATCH_POLL_INTERVAL)
            .with_compare_contents(true),
    )?;
    watcher.watch(&canonical_root, RecursiveMode::Recursive)?;

    let mut recent = HashMap::new();
    let mut known_paths = collect_known_paths(&canonical_root)?;
    loop {
        let first = tokio::select! {
            _ = cancel.cancelled() => break,
            event = raw_rx.recv() => match event {
                Some(event) => event?,
                None => break,
            },
        };
        let mut batch = vec![first];
        let deadline = tokio::time::Instant::now() + BATCH_WINDOW;
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    drop(watcher);
                    return Ok(());
                }
                _ = tokio::time::sleep_until(deadline) => break,
                event = raw_rx.recv() => match event {
                    Some(event) => batch.push(event?),
                    None => break,
                },
            }
        }

        let now = Instant::now();
        recent.retain(|_, seen| now.duration_since(*seen) < DEDUPLICATION_WINDOW);
        for mut change in adapt_events(&canonical_root, batch)? {
            reconcile_kind(&mut change, &mut known_paths);
            if recent.insert(change.clone(), now).is_some() {
                continue;
            }
            tokio::select! {
                _ = cancel.cancelled() => {
                    drop(watcher);
                    return Ok(());
                }
                result = tx.send(change) => {
                    if result.is_err() {
                        drop(watcher);
                        return Err(FilesystemCaptureError::ReceiverClosed);
                    }
                }
            }
        }
    }

    drop(watcher);
    Ok(())
}

fn reconcile_kind(change: &mut FilesystemChange, known_paths: &mut BTreeSet<String>) {
    match change.kind {
        FilesystemChangeKind::Created => {
            if !known_paths.insert(change.path.clone()) {
                change.kind = FilesystemChangeKind::Modified;
            }
        }
        FilesystemChangeKind::Modified => {
            known_paths.insert(change.path.clone());
        }
        FilesystemChangeKind::Moved => {
            if let Some(previous_path) = &change.previous_path {
                known_paths.remove(previous_path);
            }
            known_paths.insert(change.path.clone());
        }
        FilesystemChangeKind::Deleted => {
            known_paths.remove(&change.path);
        }
    }
}

fn collect_known_paths(root: &Path) -> Result<BTreeSet<String>, FilesystemCaptureError> {
    let mut known = BTreeSet::new();
    let mut directories = vec![root.to_owned()];
    while let Some(directory) = directories.pop() {
        let entries = std::fs::read_dir(&directory).map_err(|source| {
            FilesystemCaptureError::InspectPath {
                path: directory.clone(),
                source,
            }
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| FilesystemCaptureError::InspectPath {
                path: directory.clone(),
                source,
            })?;
            let path = entry.path();
            if entry
                .file_type()
                .map_err(|source| FilesystemCaptureError::InspectPath {
                    path: path.clone(),
                    source,
                })?
                .is_dir()
            {
                directories.push(path.clone());
            }
            known.insert(normalize_workspace_path(root, &path)?);
        }
    }
    Ok(known)
}

fn adapt_events(
    root: &Path,
    events: Vec<Event>,
) -> Result<Vec<FilesystemChange>, FilesystemCaptureError> {
    let mut changes = BTreeSet::new();
    let mut rename_from = VecDeque::new();
    let mut rename_to = VecDeque::new();
    let mut rename_any = Vec::new();

    for event in events {
        match event.kind {
            EventKind::Create(_) => {
                for path in event.paths {
                    changes.insert(change_for_path(
                        root,
                        FilesystemChangeKind::Created,
                        path,
                        None,
                    )?);
                }
            }
            EventKind::Modify(ModifyKind::Name(mode)) => match mode {
                RenameMode::Both if event.paths.len() >= 2 => {
                    for pair in event.paths.chunks_exact(2) {
                        changes.insert(change_for_path(
                            root,
                            FilesystemChangeKind::Moved,
                            pair[1].clone(),
                            Some(pair[0].clone()),
                        )?);
                    }
                }
                RenameMode::From => rename_from.extend(event.paths),
                RenameMode::To => rename_to.extend(event.paths),
                RenameMode::Any => rename_any.extend(event.paths),
                _ => {}
            },
            EventKind::Modify(_) => {
                for path in event.paths {
                    changes.insert(change_for_path(
                        root,
                        FilesystemChangeKind::Modified,
                        path,
                        None,
                    )?);
                }
            }
            EventKind::Remove(_) => {
                for path in event.paths {
                    changes.insert(change_for_path(
                        root,
                        FilesystemChangeKind::Deleted,
                        path,
                        None,
                    )?);
                }
            }
            _ => {}
        }
    }

    while let (Some(from), Some(to)) = (rename_from.pop_front(), rename_to.pop_front()) {
        changes.insert(change_for_path(
            root,
            FilesystemChangeKind::Moved,
            to,
            Some(from),
        )?);
    }
    for path in rename_from {
        changes.insert(change_for_path(
            root,
            FilesystemChangeKind::Deleted,
            path,
            None,
        )?);
    }
    for path in rename_to {
        changes.insert(change_for_path(
            root,
            FilesystemChangeKind::Created,
            path,
            None,
        )?);
    }
    let (mut any_to, mut any_from): (Vec<_>, Vec<_>) =
        rename_any.into_iter().partition(|path| path.exists());
    any_from.sort();
    any_to.sort();
    let paired = any_from.len().min(any_to.len());
    for (from, to) in any_from.drain(..paired).zip(any_to.drain(..paired)) {
        changes.insert(change_for_path(
            root,
            FilesystemChangeKind::Moved,
            to,
            Some(from),
        )?);
    }
    for path in any_from {
        changes.insert(change_for_path(
            root,
            FilesystemChangeKind::Deleted,
            path,
            None,
        )?);
    }
    for path in any_to {
        changes.insert(change_for_path(
            root,
            FilesystemChangeKind::Created,
            path,
            None,
        )?);
    }
    infer_single_poll_rename(&mut changes);

    let structural_paths = changes
        .iter()
        .filter(|change| {
            matches!(
                change.kind,
                FilesystemChangeKind::Created | FilesystemChangeKind::Moved
            )
        })
        .map(|change| change.path.clone())
        .collect::<BTreeSet<_>>();
    Ok(changes
        .into_iter()
        .filter(|change| {
            change.kind != FilesystemChangeKind::Modified
                || !structural_paths.contains(&change.path)
        })
        .collect())
}

fn infer_single_poll_rename(changes: &mut BTreeSet<FilesystemChange>) {
    if changes
        .iter()
        .any(|change| change.kind == FilesystemChangeKind::Moved)
    {
        return;
    }
    let created = changes
        .iter()
        .filter(|change| change.kind == FilesystemChangeKind::Created)
        .cloned()
        .collect::<Vec<_>>();
    let deleted = changes
        .iter()
        .filter(|change| change.kind == FilesystemChangeKind::Deleted)
        .cloned()
        .collect::<Vec<_>>();
    if let ([created], [deleted]) = (created.as_slice(), deleted.as_slice()) {
        if created.path == deleted.path {
            return;
        }
        let mut moved = created.clone();
        moved.kind = FilesystemChangeKind::Moved;
        moved.previous_path = Some(deleted.path.clone());
        changes.remove(created);
        changes.remove(deleted);
        changes.insert(moved);
    }
}

fn change_for_path(
    root: &Path,
    kind: FilesystemChangeKind,
    path: PathBuf,
    previous_path: Option<PathBuf>,
) -> Result<FilesystemChange, FilesystemCaptureError> {
    let normalized = normalize_workspace_path(root, &path)?;
    let previous_path = previous_path
        .map(|path| normalize_workspace_path(root, &path))
        .transpose()?;
    let (content_hash, size) = if kind == FilesystemChangeKind::Deleted {
        (None, None)
    } else {
        file_metadata(&path)?
    };

    Ok(FilesystemChange {
        kind,
        path: normalized,
        previous_path,
        content_hash,
        size,
    })
}

fn normalize_workspace_path(root: &Path, path: &Path) -> Result<String, FilesystemCaptureError> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        root.join(path)
    };
    let lexical = normalize_lexically(&absolute);
    if !lexical.starts_with(root) {
        return Err(FilesystemCaptureError::PathOutsideRoot(path.to_owned()));
    }
    if absolute.exists() {
        let resolved =
            absolute
                .canonicalize()
                .map_err(|source| FilesystemCaptureError::InspectPath {
                    path: absolute.clone(),
                    source,
                })?;
        if !resolved.starts_with(root) {
            return Err(FilesystemCaptureError::PathOutsideRoot(path.to_owned()));
        }
    }
    let relative = lexical
        .strip_prefix(root)
        .map_err(|_| FilesystemCaptureError::PathOutsideRoot(path.to_owned()))?;
    Ok(relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/"))
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                normalized.pop();
            }
            Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn file_metadata(path: &Path) -> Result<(Option<String>, Option<u64>), FilesystemCaptureError> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((None, None)),
        Err(source) => {
            return Err(FilesystemCaptureError::InspectPath {
                path: path.to_owned(),
                source,
            })
        }
    };
    if !metadata.is_file() {
        return Ok((None, Some(metadata.len())));
    }

    let mut file = File::open(path).map_err(|source| FilesystemCaptureError::InspectPath {
        path: path.to_owned(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read =
            file.read(&mut buffer)
                .map_err(|source| FilesystemCaptureError::InspectPath {
                    path: path.to_owned(),
                    source,
                })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok((
        Some(format!("{:x}", hasher.finalize())),
        Some(metadata.len()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[cfg(unix)]
    #[test]
    fn rejects_existing_paths_that_resolve_outside_the_root() {
        let temp = TempDir::new().expect("temp directory");
        let root = temp.path().join("workspace");
        let outside = temp.path().join("outside.txt");
        std::fs::create_dir(&root).expect("workspace");
        std::fs::write(&outside, b"outside").expect("outside file");
        std::os::unix::fs::symlink(&outside, root.join("escape")).expect("symlink");

        assert!(matches!(
            normalize_workspace_path(&root, &root.join("escape")),
            Err(FilesystemCaptureError::PathOutsideRoot(path)) if path == root.join("escape")
        ));
    }
}
