use std::future::Future;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::json;
use skilltape_capture::{
    capture_terminal, merge_capture_timeline, watch_workspace, CaptureOptions, FilesystemChange,
    FilesystemChangeKind, TimelineFilesystemChange,
};
use skilltape_runner::publish_directory_noreplace;
use skilltape_tape::{
    EventSource, RedactionState, TapeEvent, TapeEventKind, TapeManifest, TapeStore, TAPE_SCHEMA_V1,
};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

const CAPTURE_ERROR_EXIT_CODE: u8 = 1;
const CAPTURE_CANCELLED_EXIT_CODE: u8 = 130;
// Keep the in-memory merge finite; larger captures need an external merge design.
const MAX_CAPTURE_EVENTS: usize = 10_000;
const MAX_CAPTURE_EVENT_BYTES: usize = 16 * 1024 * 1024;

#[cfg(unix)]
static INTERRUPTED: AtomicBool = AtomicBool::new(false);
static TAPE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[cfg(unix)]
extern "C" fn handle_interrupt(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::Release);
}

#[cfg(unix)]
fn install_interrupt_handler() {
    unsafe {
        libc::signal(
            libc::SIGINT,
            handle_interrupt as *const () as libc::sighandler_t,
        );
    }
}

pub(crate) struct InterruptGuard;

impl InterruptGuard {
    pub(crate) fn install() -> Self {
        #[cfg(unix)]
        install_interrupt_handler();
        Self
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::signal(libc::SIGINT, libc::SIG_DFL);
        }
    }
}

#[derive(Debug)]
pub(crate) struct CaptureConfig {
    pub name: String,
    pub workspace: Option<PathBuf>,
    pub command: Option<String>,
    pub interactive: bool,
    pub output: Option<PathBuf>,
    pub allow_env: Vec<String>,
    pub max_output_bytes: usize,
    pub json: bool,
    pub yes: bool,
}

#[derive(Debug, Error)]
enum CaptureCommandError {
    #[error("capture name must be a safe single path component")]
    InvalidName,
    #[error("capture requires explicit confirmation; pass --yes")]
    ConfirmationRequired,
    #[error("capture workspace is not a directory: {0}")]
    InvalidWorkspace(PathBuf),
    #[error("capture output path is unsafe: {0}")]
    UnsafeOutput(PathBuf),
    #[error("capture output already exists: {0}")]
    OutputExists(PathBuf),
    #[error("capture output limit must be greater than zero")]
    InvalidOutputLimit,
    #[error("capture setup failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("capture tape failed: {0}")]
    Tape(#[from] skilltape_tape::TapeStoreError),
    #[error("capture failed: {0}")]
    Capture(#[from] skilltape_capture::CaptureError),
    #[error("filesystem capture failed: {0}")]
    Filesystem(#[from] skilltape_capture::FilesystemCaptureError),
    #[error("capture task failed: {0}")]
    Task(String),
    #[error("capture event budget exceeded; split the command into smaller captures")]
    EventBudgetExceeded,
}

#[derive(Serialize)]
struct CaptureJsonSummary {
    ok: bool,
    name: String,
    tape_path: String,
    workspace: String,
    id: String,
    event_count: u64,
    filesystem_events: usize,
    command: String,
    exit_code: u32,
    signal: Option<String>,
    cancelled: bool,
    timed_out: bool,
    output_bytes: usize,
    output_truncated: bool,
}

pub(crate) fn run(config: CaptureConfig) -> ExitCode {
    let json_output = config.json;
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            report_error(json_output, &format!("capture runtime failed: {error}"));
            return ExitCode::from(CAPTURE_ERROR_EXIT_CODE);
        }
    };

    match runtime.block_on(capture(config)) {
        Ok(summary) => {
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string(&summary).expect("capture summary serialization")
                );
            } else {
                println!(
                    "Captured {} at {} ({} events)",
                    summary.name, summary.tape_path, summary.event_count
                );
            }
            if summary.cancelled {
                ExitCode::from(CAPTURE_CANCELLED_EXIT_CODE)
            } else if summary.exit_code > 0 {
                ExitCode::from(summary.exit_code.min(u8::MAX as u32) as u8)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            report_error(json_output, &error.to_string());
            ExitCode::from(CAPTURE_ERROR_EXIT_CODE)
        }
    }
}

async fn capture(config: CaptureConfig) -> Result<CaptureJsonSummary, CaptureCommandError> {
    validate_name(&config.name)?;
    if !config.yes {
        return Err(CaptureCommandError::ConfirmationRequired);
    }
    if config.max_output_bytes == 0 {
        return Err(CaptureCommandError::InvalidOutputLimit);
    }

    let workspace = resolve_workspace(config.workspace)?;
    let use_default_output = config.output.is_none();
    let output = resolve_output(config.output, &workspace, &config.name)?;
    let interactive = config.interactive || config.command.is_none();
    let command = config
        .command
        .unwrap_or_else(|| std::env::var("SHELL").unwrap_or_else(|_| "sh".to_owned()));
    if command.is_empty() {
        return Err(CaptureCommandError::InvalidName);
    }

    let started_at_ms = now_ms();
    let id = next_tape_id(started_at_ms);
    let manifest = TapeManifest {
        schema: TAPE_SCHEMA_V1.to_owned(),
        id: id.clone(),
        started_at_ms,
        finished_at_ms: None,
        platform: std::env::consts::OS.to_owned(),
        workspace_root: workspace_name(&workspace),
        event_count: 0,
    };
    let staging = tempfile::Builder::new()
        .prefix("skilltape-capture-")
        .tempdir()?;
    let staging_root = staging.path().join("tape");
    let staging_store = TapeStore::create(&staging_root, manifest.clone())?;

    let (change_tx, change_rx) = mpsc::channel(1024);
    let watcher_cancel = CancellationToken::new();
    let (ready_tx, ready_rx) = oneshot::channel();
    let watcher_root = workspace.clone();
    let watcher_cancel_task = watcher_cancel.clone();
    let watcher_task = tokio::spawn(async move {
        let future = watch_workspace(&watcher_root, change_tx, watcher_cancel_task);
        tokio::pin!(future);
        let mut ready_tx = Some(ready_tx);
        std::future::poll_fn(|context| {
            let result = future.as_mut().poll(context);
            if let Some(ready_tx) = ready_tx.take() {
                let _ = ready_tx.send(());
            }
            result
        })
        .await
    });
    ready_rx
        .await
        .map_err(|_| CaptureCommandError::Task("filesystem watcher did not start".to_owned()))?;

    let cancel = CancellationToken::new();
    let collector = tokio::spawn(collect_changes(change_rx, cancel.clone()));
    let capture_future = capture_terminal(
        CaptureOptions {
            command: command.clone(),
            args: Vec::new(),
            workspace: workspace.clone(),
            env_allowlist: config.allow_env,
            output_limit: config.max_output_bytes,
            interactive,
        },
        staging_store,
        cancel.clone(),
    );
    tokio::pin!(capture_future);
    let interrupt = wait_for_interrupt();
    tokio::pin!(interrupt);
    let capture_result = tokio::select! {
        result = &mut capture_future => result,
        _ = &mut interrupt => {
            cancel.cancel();
            (&mut capture_future).await
        }
    };
    let capture_cancelled = cancel.is_cancelled();

    // PollWatcher checks at 50 ms intervals and batches raw events for 40 ms.
    // Three complete poll-and-batch windows leave a bounded margin for macOS
    // CI scheduling before stopping the watcher for a short-lived command.
    tokio::time::sleep(Duration::from_millis(300)).await;
    watcher_cancel.cancel();
    let watcher_result = watcher_task
        .await
        .map_err(|error| CaptureCommandError::Task(error.to_string()))?;
    let filesystem_events = collector
        .await
        .map_err(|error| CaptureCommandError::Task(error.to_string()))??;
    if let Err(error) = watcher_result {
        if !capture_cancelled {
            return Err(error.into());
        }
    }
    let capture_summary = match capture_result {
        Ok(summary) => summary,
        Err(_error) if capture_cancelled => skilltape_capture::CaptureSummary {
            exit_code: 130,
            signal: Some("SIGINT".to_owned()),
            cancelled: true,
            timed_out: false,
            output_bytes: 0,
            output_truncated: false,
            terminal_size: Default::default(),
        },
        Err(error) => {
            return Err(error.into());
        }
    };

    let staging_store = TapeStore::open(&staging_root)?;
    let filesystem_count = filesystem_events.len();
    let tape_events = staging_store
        .read_events_bounded(
            MAX_CAPTURE_EVENT_BYTES as u64,
            (MAX_CAPTURE_EVENTS - filesystem_count) as u64,
        )
        .and_then(|events| events.collect::<Result<Vec<_>, _>>())?;
    let merged = merge_capture_timeline(filesystem_events, tape_events, Duration::from_millis(40));
    if use_default_output {
        validate_default_output(&output, &workspace)?;
    }
    let final_manifest = publish_capture_timeline(
        &output,
        manifest,
        merged.into_iter().flat_map(|batch| batch.events),
    )?;

    Ok(CaptureJsonSummary {
        ok: true,
        name: config.name,
        tape_path: output.to_string_lossy().into_owned(),
        workspace: workspace.to_string_lossy().into_owned(),
        id,
        event_count: final_manifest.event_count,
        filesystem_events: filesystem_count,
        command,
        exit_code: capture_summary.exit_code,
        signal: capture_summary.signal,
        cancelled: capture_summary.cancelled,
        timed_out: capture_summary.timed_out,
        output_bytes: capture_summary.output_bytes,
        output_truncated: capture_summary.output_truncated,
    })
}

fn publish_capture_timeline(
    output: &Path,
    manifest: TapeManifest,
    events: impl IntoIterator<Item = skilltape_capture::TimelineEvent>,
) -> Result<TapeManifest, CaptureCommandError> {
    // Finish beside the destination, then use the existing no-replace publisher.
    let parent = prepare_capture_parent(output)?;
    let output = parent.join(
        output
            .file_name()
            .ok_or_else(|| CaptureCommandError::UnsafeOutput(output.to_owned()))?,
    );
    let completed = tempfile::Builder::new()
        .prefix(".skilltape-completed-")
        .tempdir_in(&parent)?;
    let completed_root = completed.path().join("tape");
    let final_store = TapeStore::create(&completed_root, manifest)?;
    for (sequence, event) in events.into_iter().enumerate() {
        let event = match event {
            skilltape_capture::TimelineEvent::Tape(mut event) => {
                event.sequence = sequence as u64;
                event
            }
            skilltape_capture::TimelineEvent::Filesystem(event) => TapeEvent {
                sequence: sequence as u64,
                occurred_at_ms: event.occurred_at_ms,
                kind: TapeEventKind::FilesystemChanged,
                source: EventSource::Filesystem,
                payload: filesystem_payload(&event.change),
                redaction: RedactionState::Unredacted,
            },
        };
        final_store.append(&event)?;
    }
    let finished = final_store.finish(now_ms())?;
    publish_directory_noreplace(&completed_root, &output)?;
    Ok(finished)
}

fn prepare_capture_parent(output: &Path) -> Result<PathBuf, CaptureCommandError> {
    if output
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(CaptureCommandError::UnsafeOutput(output.to_owned()));
    }
    let parent = output
        .parent()
        .ok_or_else(|| CaptureCommandError::UnsafeOutput(output.to_owned()))?;
    let mut ancestor = parent.to_owned();
    while !ancestor.exists() {
        if !ancestor.pop() {
            return Err(CaptureCommandError::UnsafeOutput(output.to_owned()));
        }
    }
    // The caller selects the output location. Anchor new descendants to its
    // existing canonical directory instead of creating from an unchecked path.
    let selected_root = ancestor.canonicalize()?;
    let relative = parent
        .strip_prefix(&ancestor)
        .map_err(|_| CaptureCommandError::UnsafeOutput(output.to_owned()))?;
    if !selected_root.is_dir()
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(CaptureCommandError::UnsafeOutput(output.to_owned()));
    }
    let resolved = selected_root.join(relative);
    if !resolved.starts_with(&selected_root) {
        return Err(CaptureCommandError::UnsafeOutput(output.to_owned()));
    }
    std::fs::create_dir_all(&resolved)?;
    let canonical = resolved.canonicalize()?;
    if !canonical.starts_with(&selected_root) {
        return Err(CaptureCommandError::UnsafeOutput(output.to_owned()));
    }
    Ok(canonical)
}

async fn wait_for_interrupt() {
    #[cfg(unix)]
    loop {
        if INTERRUPTED.swap(false, Ordering::AcqRel) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    #[cfg(not(unix))]
    std::future::pending::<()>().await;
}

async fn collect_changes(
    mut receiver: mpsc::Receiver<FilesystemChange>,
    cancel: CancellationToken,
) -> Result<Vec<TimelineFilesystemChange>, CaptureCommandError> {
    let mut events = Vec::new();
    let mut metadata_bytes = 0_usize;
    while let Some(change) = receiver.recv().await {
        // Charge owned string capacity, not just text length. The event ceiling
        // separately bounds Vec growth; this is not a process RSS limit.
        let bytes = std::mem::size_of::<TimelineFilesystemChange>()
            .saturating_add(change.path.capacity())
            .saturating_add(change.previous_path.as_ref().map_or(0, String::capacity))
            .saturating_add(change.content_hash.as_ref().map_or(0, String::capacity));
        metadata_bytes = metadata_bytes.saturating_add(bytes);
        if events.len() >= MAX_CAPTURE_EVENTS || metadata_bytes > MAX_CAPTURE_EVENT_BYTES {
            cancel.cancel();
            return Err(CaptureCommandError::EventBudgetExceeded);
        }
        events.push(TimelineFilesystemChange {
            occurred_at_ms: now_ms(),
            change,
        });
    }
    Ok(events)
}

fn validate_name(name: &str) -> Result<(), CaptureCommandError> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || !matches!(character, 'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.')
        })
    {
        return Err(CaptureCommandError::InvalidName);
    }
    Ok(())
}

fn resolve_workspace(path: Option<PathBuf>) -> Result<PathBuf, CaptureCommandError> {
    let path = path.unwrap_or(std::env::current_dir()?);
    let canonical = path
        .canonicalize()
        .map_err(|_| CaptureCommandError::InvalidWorkspace(path.clone()))?;
    if !canonical.is_dir() {
        return Err(CaptureCommandError::InvalidWorkspace(canonical));
    }
    Ok(canonical)
}

fn resolve_output(
    path: Option<PathBuf>,
    workspace: &Path,
    name: &str,
) -> Result<PathBuf, CaptureCommandError> {
    let is_default = path.is_none();
    let output = path.unwrap_or_else(|| workspace.join(".skilltape/tapes").join(name));
    if output
        .components()
        .any(|component| matches!(component, Component::ParentDir))
        || output.file_name().is_none()
    {
        return Err(CaptureCommandError::UnsafeOutput(output));
    }
    let output = if output.is_absolute() {
        output
    } else {
        std::env::current_dir()?.join(output)
    };
    if output == workspace || output.exists() {
        return if output == workspace {
            Err(CaptureCommandError::UnsafeOutput(output))
        } else {
            Err(CaptureCommandError::OutputExists(output))
        };
    }
    if is_default {
        validate_default_output(&output, workspace)?;
    }
    Ok(output)
}

fn validate_default_output(output: &Path, workspace: &Path) -> Result<(), CaptureCommandError> {
    let resolved = canonicalize_nearest_existing_ancestor(output)
        .map_err(|_| CaptureCommandError::UnsafeOutput(output.to_owned()))?;
    if resolved.starts_with(workspace) {
        Ok(())
    } else {
        Err(CaptureCommandError::UnsafeOutput(output.to_owned()))
    }
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

fn workspace_name(workspace: &Path) -> String {
    workspace
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "workspace".to_owned())
}

fn filesystem_payload(change: &FilesystemChange) -> serde_json::Value {
    json!({
        "kind": match change.kind {
            FilesystemChangeKind::Created => "created",
            FilesystemChangeKind::Modified => "modified",
            FilesystemChangeKind::Moved => "moved",
            FilesystemChangeKind::Deleted => "deleted",
        },
        "path": change.path,
        "previous_path": change.previous_path,
        "content_hash": change.content_hash,
        "size": change.size,
    })
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

fn next_tape_id(started_at_ms: u64) -> String {
    let sequence = TAPE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!(
        "tape_{started_at_ms:020}-{:020}-{sequence:020}",
        std::process::id()
    )
}

fn report_error(json_output: bool, message: &str) {
    if json_output {
        println!("{}", serde_json::json!({"ok": false, "error": message}));
    }
    eprintln!("{message}");
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    fn manifest() -> TapeManifest {
        TapeManifest {
            schema: TAPE_SCHEMA_V1.to_owned(),
            id: "synthetic-budget".to_owned(),
            started_at_ms: 1,
            finished_at_ms: None,
            platform: "synthetic".to_owned(),
            workspace_root: "workspace".to_owned(),
            event_count: 0,
        }
    }

    #[test]
    fn failed_finalization_never_publishes_a_partial_tape() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("tape");
        let mut invalid = manifest();
        invalid.finished_at_ms = Some(2);
        assert!(publish_capture_timeline(&output, invalid, []).is_err());
        assert!(!output.exists());
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[test]
    fn publication_preserves_a_destination_created_by_another_writer() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("tape");
        std::fs::create_dir(&output).unwrap();
        std::fs::write(output.join("preserved"), "synthetic original").unwrap();
        assert!(publish_capture_timeline(&output, manifest(), []).is_err());
        assert_eq!(
            std::fs::read_to_string(output.join("preserved")).unwrap(),
            "synthetic original"
        );
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
    }

    #[test]
    fn publication_rejects_parent_traversal_before_creating_directories() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("selected/../outside/tape");
        assert!(publish_capture_timeline(&output, manifest(), []).is_err());
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[test]
    fn publication_supports_new_nested_output_parents() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("nested/output/tape");
        publish_capture_timeline(&output, manifest(), []).unwrap();
        assert!(output.join("manifest.json").is_file());
        assert_eq!(
            std::fs::read_dir(output.parent().unwrap()).unwrap().count(),
            1
        );
    }

    fn change(path: String) -> FilesystemChange {
        FilesystemChange {
            kind: FilesystemChangeKind::Created,
            path,
            previous_path: None,
            content_hash: None,
            size: Some(1),
        }
    }

    #[tokio::test]
    async fn filesystem_collection_has_a_finite_event_budget() {
        let (sender, receiver) = mpsc::channel(8);
        let producer = tokio::spawn(async move {
            for index in 0..10_001 {
                if sender.send(change(format!("file-{index}"))).await.is_err() {
                    break;
                }
            }
        });
        let cancel = CancellationToken::new();
        let result = collect_changes(receiver, cancel.clone()).await;
        producer.await.unwrap();
        assert!(matches!(
            result,
            Err(CaptureCommandError::EventBudgetExceeded)
        ));
        assert!(cancel.is_cancelled());
    }

    #[tokio::test]
    async fn filesystem_metadata_has_a_byte_budget() {
        let (sender, receiver) = mpsc::channel(1);
        sender
            .send(change("x".repeat(MAX_CAPTURE_EVENT_BYTES)))
            .await
            .unwrap();
        drop(sender);
        let cancel = CancellationToken::new();
        assert!(matches!(
            collect_changes(receiver, cancel.clone()).await,
            Err(CaptureCommandError::EventBudgetExceeded)
        ));
        assert!(cancel.is_cancelled());
    }

    #[tokio::test]
    async fn short_paths_cannot_hide_large_allocations() {
        let (sender, receiver) = mpsc::channel(1);
        let mut path = String::with_capacity(MAX_CAPTURE_EVENT_BYTES);
        path.push('x');
        sender.send(change(path)).await.unwrap();
        drop(sender);
        let cancel = CancellationToken::new();
        assert!(matches!(
            collect_changes(receiver, cancel.clone()).await,
            Err(CaptureCommandError::EventBudgetExceeded)
        ));
        assert!(cancel.is_cancelled());
    }

    #[tokio::test]
    async fn ordinary_filesystem_changes_are_preserved_without_cancelling() {
        let (sender, receiver) = mpsc::channel(1);
        sender.send(change("result.txt".to_owned())).await.unwrap();
        drop(sender);
        let cancel = CancellationToken::new();
        let events = collect_changes(receiver, cancel.clone()).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].change.path, "result.txt");
        assert!(!cancel.is_cancelled());
    }
}
