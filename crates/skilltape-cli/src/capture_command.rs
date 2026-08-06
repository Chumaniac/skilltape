use std::future::Future;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::json;
use skilltape_capture::{
    capture_terminal, merge_capture_timeline, watch_workspace, CaptureOptions, FilesystemChange,
    FilesystemChangeKind, TimelineFilesystemChange,
};
use skilltape_tape::{
    EventSource, RedactionState, TapeEvent, TapeEventKind, TapeManifest, TapeStore, TAPE_SCHEMA_V1,
};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

const CAPTURE_ERROR_EXIT_CODE: u8 = 1;
const CAPTURE_CANCELLED_EXIT_CODE: u8 = 130;

#[cfg(unix)]
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

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
    let command = config
        .command
        .unwrap_or_else(|| std::env::var("SHELL").unwrap_or_else(|_| "sh".to_owned()));
    if command.is_empty() {
        return Err(CaptureCommandError::InvalidName);
    }

    let started_at_ms = now_ms();
    let id = format!("tape_{}", config.name);
    let manifest = TapeManifest {
        schema: TAPE_SCHEMA_V1.to_owned(),
        id: id.clone(),
        started_at_ms,
        finished_at_ms: None,
        platform: std::env::consts::OS.to_owned(),
        workspace_root: workspace_name(&workspace),
        event_count: 0,
    };
    let staging_root = std::env::temp_dir().join(format!(
        "skilltape-capture-{}-{}",
        std::process::id(),
        started_at_ms
    ));
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

    let collector = tokio::spawn(collect_changes(change_rx));
    let cancel = CancellationToken::new();
    let capture_future = capture_terminal(
        CaptureOptions {
            command: command.clone(),
            args: Vec::new(),
            workspace: workspace.clone(),
            env_allowlist: config.allow_env,
            output_limit: config.max_output_bytes,
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

    // PollWatcher checks at 50 ms intervals; allow one poll to flush metadata
    // events generated by a short-lived command before stopping it.
    tokio::time::sleep(Duration::from_millis(75)).await;
    watcher_cancel.cancel();
    let watcher_result = watcher_task
        .await
        .map_err(|error| CaptureCommandError::Task(error.to_string()))?;
    let filesystem_events = collector
        .await
        .map_err(|error| CaptureCommandError::Task(error.to_string()))?;
    if let Err(error) = watcher_result {
        if !capture_cancelled {
            let _ = std::fs::remove_dir_all(&staging_root);
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
            let _ = std::fs::remove_dir_all(&staging_root);
            return Err(error.into());
        }
    };

    let staging_store = TapeStore::open(&staging_root)?;
    let tape_events = staging_store
        .read_events()?
        .collect::<Result<Vec<_>, _>>()?;
    let merged = merge_capture_timeline(
        filesystem_events
            .iter()
            .cloned()
            .map(|event| TimelineFilesystemChange {
                occurred_at_ms: event.occurred_at_ms,
                change: event.change,
            }),
        tape_events,
        Duration::from_millis(40),
    );
    if use_default_output {
        if let Err(error) = validate_default_output(&output, &workspace) {
            let _ = std::fs::remove_dir_all(&staging_root);
            return Err(error);
        }
    }
    let final_store = match TapeStore::create(&output, manifest) {
        Ok(store) => store,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging_root);
            return Err(error.into());
        }
    };
    for event in merged.into_iter().flat_map(|batch| batch.events) {
        let event = match event {
            skilltape_capture::TimelineEvent::Tape(mut event) => {
                event.sequence = final_store.read_manifest()?.event_count;
                event
            }
            skilltape_capture::TimelineEvent::Filesystem(event) => TapeEvent {
                sequence: final_store.read_manifest()?.event_count,
                occurred_at_ms: event.occurred_at_ms,
                kind: TapeEventKind::FilesystemChanged,
                source: EventSource::Filesystem,
                payload: filesystem_payload(&event.change),
                redaction: RedactionState::Unredacted,
            },
        };
        if let Err(error) = final_store.append(&event) {
            let _ = std::fs::remove_dir_all(&output);
            let _ = std::fs::remove_dir_all(&staging_root);
            return Err(error.into());
        }
    }
    let final_manifest = final_store.finish(now_ms())?;
    let _ = std::fs::remove_dir_all(&staging_root);

    Ok(CaptureJsonSummary {
        ok: true,
        name: config.name,
        tape_path: output.to_string_lossy().into_owned(),
        workspace: workspace.to_string_lossy().into_owned(),
        id,
        event_count: final_manifest.event_count,
        filesystem_events: filesystem_events.len(),
        command,
        exit_code: capture_summary.exit_code,
        signal: capture_summary.signal,
        cancelled: capture_summary.cancelled,
        timed_out: capture_summary.timed_out,
        output_bytes: capture_summary.output_bytes,
        output_truncated: capture_summary.output_truncated,
    })
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
) -> Vec<TimelineFilesystemChange> {
    let mut events = Vec::new();
    while let Some(change) = receiver.recv().await {
        events.push(TimelineFilesystemChange {
            occurred_at_ms: now_ms(),
            change,
        });
    }
    events
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

fn report_error(json_output: bool, message: &str) {
    if json_output {
        println!("{}", serde_json::json!({"ok": false, "error": message}));
    }
    eprintln!("{message}");
}
