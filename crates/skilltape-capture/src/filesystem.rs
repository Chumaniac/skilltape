use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::event::{ModifyKind, RenameMode};
use notify::{Config, Event, EventKind, PollWatcher, RecursiveMode, Watcher};
use sha2::{Digest, Sha256};
use skilltape_tape::TapeEvent;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

const BATCH_WINDOW: Duration = Duration::from_millis(40);
const DEDUPLICATION_WINDOW: Duration = Duration::from_millis(250);
const RAW_EVENT_CAPACITY: usize = 64;
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

/// A filesystem change with the capture timestamp needed for timeline merging.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineFilesystemChange {
    /// Capture timestamp in milliseconds since the Unix epoch.
    pub occurred_at_ms: u64,
    /// The captured workspace change.
    pub change: FilesystemChange,
}

/// One timestamped event in a merged capture timeline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimelineEvent {
    /// A filesystem event captured from the workspace watcher.
    Filesystem(TimelineFilesystemChange),
    /// A terminal or other event already recorded in a tape.
    Tape(TapeEvent),
}

impl TimelineEvent {
    /// Returns the event timestamp in milliseconds since the Unix epoch.
    pub fn occurred_at_ms(&self) -> u64 {
        match self {
            Self::Filesystem(event) => event.occurred_at_ms,
            Self::Tape(event) => event.occurred_at_ms,
        }
    }
}

/// A deterministic group of events captured within one merge window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineBatch {
    /// Timestamp of the earliest event in this batch.
    pub start_at_ms: u64,
    /// Events ordered by timestamp and deterministic tie-breakers.
    pub events: Vec<TimelineEvent>,
}

/// Merges filesystem and tape events into deterministic time-window batches.
///
/// Events are first ordered by timestamp. A new event joins the current batch
/// when it is no more than `window` milliseconds after that batch's first
/// event; otherwise a new batch starts. Equal timestamps are ordered with
/// filesystem events before tape events, then by their stable event keys.
pub fn merge_capture_timeline(
    filesystem: impl IntoIterator<Item = TimelineFilesystemChange>,
    tape_events: impl IntoIterator<Item = TapeEvent>,
    window: Duration,
) -> Vec<TimelineBatch> {
    let mut events = filesystem
        .into_iter()
        .map(TimelineEvent::Filesystem)
        .chain(tape_events.into_iter().map(TimelineEvent::Tape))
        .collect::<Vec<_>>();
    events.sort_by(compare_timeline_events);

    let window_ms = window.as_millis().min(u64::MAX as u128) as u64;
    let mut batches: Vec<TimelineBatch> = Vec::new();
    for event in events {
        if let Some(batch) = batches.last_mut() {
            if event.occurred_at_ms().saturating_sub(batch.start_at_ms) <= window_ms {
                batch.events.push(event);
                continue;
            }
        }
        batches.push(TimelineBatch {
            start_at_ms: event.occurred_at_ms(),
            events: vec![event],
        });
    }
    batches
}

fn compare_timeline_events(left: &TimelineEvent, right: &TimelineEvent) -> Ordering {
    left.occurred_at_ms()
        .cmp(&right.occurred_at_ms())
        .then_with(|| timeline_source_rank(left).cmp(&timeline_source_rank(right)))
        .then_with(|| match (left, right) {
            (TimelineEvent::Filesystem(left), TimelineEvent::Filesystem(right)) => {
                left.change.cmp(&right.change)
            }
            (TimelineEvent::Tape(left), TimelineEvent::Tape(right)) => left
                .sequence
                .cmp(&right.sequence)
                .then_with(|| format!("{:?}", left.kind).cmp(&format!("{:?}", right.kind)))
                .then_with(|| format!("{:?}", left.source).cmp(&format!("{:?}", right.source)))
                .then_with(|| left.payload.to_string().cmp(&right.payload.to_string()))
                .then_with(|| {
                    format!("{:?}", left.redaction).cmp(&format!("{:?}", right.redaction))
                }),
            _ => Ordering::Equal,
        })
}

fn timeline_source_rank(event: &TimelineEvent) -> u8 {
    match event {
        TimelineEvent::Filesystem(_) => 0,
        TimelineEvent::Tape(_) => 1,
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
    /// The bounded raw notify queue filled while capture was processing events.
    #[error("filesystem raw event queue overflowed at capacity {capacity}")]
    RawEventOverflow { capacity: usize },
}

struct RawEventState {
    overflowed: AtomicBool,
    notify: Notify,
}

impl RawEventState {
    fn is_overflowed(&self) -> bool {
        self.overflowed.load(AtomicOrdering::Acquire)
    }

    fn overflow_error(&self) -> FilesystemCaptureError {
        FilesystemCaptureError::RawEventOverflow {
            capacity: RAW_EVENT_CAPACITY,
        }
    }
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
    let (raw_tx, mut raw_rx) = mpsc::channel(RAW_EVENT_CAPACITY);
    let raw_state = Arc::new(RawEventState {
        overflowed: AtomicBool::new(false),
        notify: Notify::new(),
    });
    let callback_state = Arc::clone(&raw_state);
    let mut watcher = PollWatcher::new(
        move |event| {
            enqueue_raw_event(&raw_tx, &callback_state, event);
        },
        Config::default()
            .with_poll_interval(WATCH_POLL_INTERVAL)
            .with_compare_contents(true),
    )?;
    watcher.watch(&canonical_root, RecursiveMode::Recursive)?;

    let mut recent = HashMap::new();
    let mut known_paths = collect_known_paths(&canonical_root)?;
    loop {
        if raw_state.is_overflowed() {
            drop(watcher);
            return Err(raw_state.overflow_error());
        }
        let first = tokio::select! {
            _ = cancel.cancelled() => break,
            _ = raw_state.notify.notified() => {
                drop(watcher);
                return Err(raw_state.overflow_error());
            }
            event = raw_rx.recv() => match event {
                Some(event) => event?,
                None => break,
            },
        };
        let Some(batch) = collect_batch(&mut raw_rx, first, &cancel, &raw_state).await? else {
            drop(watcher);
            return Ok(());
        };

        let now = Instant::now();
        recent.retain(|_, seen| now.duration_since(*seen) < DEDUPLICATION_WINDOW);
        for mut change in adapt_events(&canonical_root, batch)? {
            reconcile_kind(&mut change, &mut known_paths);
            if recent.insert(change.clone(), now).is_some() {
                continue;
            }
            if raw_state.is_overflowed() {
                drop(watcher);
                return Err(raw_state.overflow_error());
            }
            if let Err(error) = send_change(&tx, change, &cancel, Some(&raw_state)).await {
                drop(watcher);
                return Err(error);
            }
        }
    }

    drop(watcher);
    Ok(())
}

fn enqueue_raw_event(
    raw_tx: &mpsc::Sender<Result<Event, notify::Error>>,
    raw_state: &RawEventState,
    event: Result<Event, notify::Error>,
) {
    if let Err(error) = raw_tx.try_send(event) {
        if matches!(error, mpsc::error::TrySendError::Full(_)) {
            raw_state.overflowed.store(true, AtomicOrdering::Release);
            raw_state.notify.notify_one();
        }
    }
}

async fn collect_batch(
    raw_rx: &mut mpsc::Receiver<Result<Event, notify::Error>>,
    first: Event,
    cancel: &CancellationToken,
    raw_state: &RawEventState,
) -> Result<Option<Vec<Event>>, FilesystemCaptureError> {
    let mut batch = vec![first];
    let deadline = tokio::time::Instant::now() + BATCH_WINDOW;
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(None),
            _ = raw_state.notify.notified() => return Err(raw_state.overflow_error()),
            _ = tokio::time::sleep_until(deadline) => return Ok(Some(batch)),
            event = raw_rx.recv() => match event {
                Some(event) => batch.push(event?),
                None => return Ok(Some(batch)),
            },
        }
    }
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
    if let ([to], [from]) = (any_to.as_slice(), any_from.as_slice()) {
        changes.insert(change_for_path(
            root,
            FilesystemChangeKind::Moved,
            to.clone(),
            Some(from.clone()),
        )?);
    } else {
        any_from.sort();
        any_to.sort();
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
    let resolved = canonicalize_nearest_existing_ancestor(&absolute).map_err(|source| {
        FilesystemCaptureError::InspectPath {
            path: absolute.clone(),
            source,
        }
    })?;
    if !resolved.starts_with(root) {
        return Err(FilesystemCaptureError::PathOutsideRoot(path.to_owned()));
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

fn canonicalize_nearest_existing_ancestor(path: &Path) -> std::io::Result<PathBuf> {
    let mut candidate = path.to_owned();
    loop {
        match candidate.canonicalize() {
            Ok(resolved) => return Ok(resolved),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !candidate.pop() {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
    }
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
    file_metadata_with_opener(path, |path| File::open(path))
}

fn file_metadata_with_opener(
    path: &Path,
    open: impl FnOnce(&Path) -> std::io::Result<File>,
) -> Result<(Option<String>, Option<u64>), FilesystemCaptureError> {
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

    let mut file = match open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((None, None)),
        Err(source) => {
            return Err(FilesystemCaptureError::InspectPath {
                path: path.to_owned(),
                source,
            })
        }
    };
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

async fn send_change(
    tx: &mpsc::Sender<FilesystemChange>,
    change: FilesystemChange,
    cancel: &CancellationToken,
    raw_state: Option<&RawEventState>,
) -> Result<(), FilesystemCaptureError> {
    if let Some(raw_state) = raw_state {
        return send_change_with_overflow(tx, change, cancel, raw_state).await;
    }
    tokio::select! {
        _ = cancel.cancelled() => Ok(()),
        result = tx.send(change) => result.map_err(|_| FilesystemCaptureError::ReceiverClosed),
    }
}

async fn send_change_with_overflow(
    tx: &mpsc::Sender<FilesystemChange>,
    change: FilesystemChange,
    cancel: &CancellationToken,
    raw_state: &RawEventState,
) -> Result<(), FilesystemCaptureError> {
    tokio::select! {
        _ = cancel.cancelled() => Ok(()),
        _ = raw_state.notify.notified() => Err(raw_state.overflow_error()),
        result = tx.send(change) => result.map_err(|_| FilesystemCaptureError::ReceiverClosed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn rename_any_does_not_pair_multiple_existing_and_missing_paths() {
        let temp = TempDir::new().expect("temp directory");
        let root = temp.path().join("workspace");
        std::fs::create_dir(&root).expect("workspace");
        let root = root.canonicalize().expect("canonical workspace");
        let existing_a = root.join("new-a.txt");
        let existing_b = root.join("new-b.txt");
        std::fs::write(&existing_a, b"a").expect("existing a");
        std::fs::write(&existing_b, b"b").expect("existing b");

        let changes = adapt_events(
            &root,
            vec![Event {
                kind: EventKind::Modify(ModifyKind::Name(RenameMode::Any)),
                paths: vec![
                    root.join("old-a.txt"),
                    existing_a.clone(),
                    root.join("old-b.txt"),
                    existing_b.clone(),
                ],
                attrs: Default::default(),
            }],
        )
        .expect("adapt rename event");

        assert_eq!(
            changes
                .iter()
                .map(|change| (change.kind, change.path.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (FilesystemChangeKind::Created, "new-a.txt"),
                (FilesystemChangeKind::Created, "new-b.txt"),
                (FilesystemChangeKind::Deleted, "old-a.txt"),
                (FilesystemChangeKind::Deleted, "old-b.txt"),
            ]
        );
        assert!(changes.iter().all(|change| change.previous_path.is_none()));
    }

    #[test]
    fn treats_disappearance_between_metadata_and_open_as_missing() {
        let temp = TempDir::new().expect("temp directory");
        let file = temp.path().join("vanishing.txt");
        std::fs::write(&file, b"vanish").expect("file");

        let metadata = file_metadata_with_opener(&file, |path| {
            std::fs::remove_file(path).expect("remove between metadata and open");
            File::open(path)
        })
        .expect("disappearance is transient");

        assert_eq!(metadata, (None, None));
    }

    #[test]
    fn rejects_lexical_paths_outside_the_root() {
        let temp = TempDir::new().expect("temp directory");
        let root = temp.path().join("workspace");
        std::fs::create_dir(&root).expect("workspace");

        assert!(matches!(
            normalize_workspace_path(&root, &root.join("..")),
            Err(FilesystemCaptureError::PathOutsideRoot(path)) if path == root.join("..")
        ));
    }

    #[tokio::test]
    async fn blocked_output_send_observes_cancellation() {
        let (tx, mut rx) = mpsc::channel(1);
        tx.send(test_change("already-buffered"))
            .await
            .expect("fill output channel");
        let cancel = CancellationToken::new();
        let task_tx = tx.clone();
        let task_cancel = cancel.clone();
        let task = tokio::spawn(async move {
            send_change(&task_tx, test_change("blocked"), &task_cancel, None).await
        });

        tokio::task::yield_now().await;
        cancel.cancel();
        assert!(tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("blocked send is cancellable")
            .expect("send task joins")
            .is_ok());
        assert_eq!(
            rx.recv().await.expect("buffered event").path,
            "already-buffered"
        );
    }

    #[tokio::test]
    async fn cancellation_during_batching_returns_without_waiting_for_the_window() {
        let (raw_tx, mut raw_rx) = mpsc::channel(1);
        let raw_state = RawEventState {
            overflowed: AtomicBool::new(false),
            notify: Notify::new(),
        };
        let cancel = CancellationToken::new();
        let task_cancel = cancel.clone();
        let task = tokio::spawn(async move {
            collect_batch(&mut raw_rx, test_event(), &task_cancel, &raw_state).await
        });

        tokio::task::yield_now().await;
        cancel.cancel();
        assert!(tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("batch cancellation is prompt")
            .expect("batch task joins")
            .expect("batch returns cleanly")
            .is_none());
        drop(raw_tx);
    }

    #[tokio::test]
    async fn raw_queue_overflow_is_reported_explicitly() {
        let (raw_tx, _raw_rx) = mpsc::channel(1);
        let raw_state = RawEventState {
            overflowed: AtomicBool::new(false),
            notify: Notify::new(),
        };

        enqueue_raw_event(&raw_tx, &raw_state, Ok(test_event()));
        enqueue_raw_event(&raw_tx, &raw_state, Ok(test_event()));

        assert!(raw_state.is_overflowed());
        assert!(matches!(
            raw_state.overflow_error(),
            FilesystemCaptureError::RawEventOverflow {
                capacity: RAW_EVENT_CAPACITY
            }
        ));
    }

    fn test_change(path: &str) -> FilesystemChange {
        FilesystemChange {
            kind: FilesystemChangeKind::Created,
            path: path.to_owned(),
            previous_path: None,
            content_hash: None,
            size: None,
        }
    }

    fn test_event() -> Event {
        Event {
            kind: EventKind::Create(notify::event::CreateKind::Any),
            paths: Vec::new(),
            attrs: Default::default(),
        }
    }

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

    #[cfg(unix)]
    #[test]
    fn rejects_nonexistent_descendants_through_outside_symlinks() {
        let temp = TempDir::new().expect("temp directory");
        let root = temp.path().join("workspace");
        let outside = temp.path().join("outside");
        let path = root.join("escape/new-file/descendant.txt");
        std::fs::create_dir(&root).expect("workspace");
        std::fs::create_dir(&outside).expect("outside directory");
        std::os::unix::fs::symlink(&outside, root.join("escape")).expect("symlink");

        assert!(matches!(
            normalize_workspace_path(&root, &path),
            Err(FilesystemCaptureError::PathOutsideRoot(rejected)) if rejected == path
        ));
    }
}
