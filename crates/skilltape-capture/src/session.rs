use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use skilltape_tape::{
    EventSource, RedactionState, TapeEvent, TapeEventKind, TapeStore, TapeStoreError,
};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::environment::snapshot_environment;
use crate::pty::{PortablePtyAdapter, PtyAdapter, PtyRequest, PtyRunResult};
use crate::{redact_text, RedactionConfig};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaptureOptions {
    pub command: String,
    pub args: Vec<String>,
    pub workspace: PathBuf,
    pub env_allowlist: Vec<String>,
    pub output_limit: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSize {
    pub rows: u16,
    pub cols: u16,
    pub pixel_width: u16,
    pub pixel_height: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaptureSummary {
    pub exit_code: u32,
    pub signal: Option<String>,
    pub cancelled: bool,
    pub output_bytes: usize,
    pub output_truncated: bool,
    pub terminal_size: TerminalSize,
}

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("capture command must not be empty")]
    EmptyCommand,
    #[error("capture workspace is not a directory: {0}")]
    InvalidWorkspace(PathBuf),
    #[error("PTY error: {0}")]
    Pty(String),
    #[error("I/O error: {0}")]
    Io(#[source] std::io::Error),
    #[error("tape store error: {0}")]
    Store(#[from] TapeStoreError),
    #[error("capture worker failed: {0}")]
    Worker(String),
}

pub async fn capture_terminal(
    options: CaptureOptions,
    store: TapeStore,
    cancel: CancellationToken,
) -> Result<CaptureSummary, CaptureError> {
    capture_terminal_with_adapter(options, store, cancel, PortablePtyAdapter).await
}

pub(crate) async fn capture_terminal_with_adapter<A: PtyAdapter>(
    options: CaptureOptions,
    store: TapeStore,
    cancel: CancellationToken,
    adapter: A,
) -> Result<CaptureSummary, CaptureError> {
    validate_options(&options)?;
    let started_at = now_ms();
    let mut sequence = store.read_manifest()?.event_count;
    let terminal_size = TerminalSize::default();
    let redaction_config = RedactionConfig {
        max_output_bytes: options.output_limit,
        ..RedactionConfig::default()
    };
    let metadata_redaction_config = RedactionConfig::default();
    let command = redact_text(&options.command, &metadata_redaction_config);
    let args = options
        .args
        .iter()
        .map(|argument| redact_text(argument, &metadata_redaction_config).text)
        .collect::<Vec<_>>();
    let environment = snapshot_environment(&options.env_allowlist);
    let environment = environment
        .variables
        .into_values()
        .map(|item| {
            json!({
                "name": item.name,
                "original_bytes": item.original_bytes,
                "sha256": item.sha256,
            })
        })
        .collect::<Vec<_>>();

    append_event(
        &store,
        &mut sequence,
        started_at,
        TapeEventKind::SessionStarted,
        EventSource::System,
        json!({
            "cwd": options.workspace,
            "environment": environment,
            "terminal_size": terminal_size_json(terminal_size),
        }),
        RedactionState::Redacted,
    )?;
    append_event(
        &store,
        &mut sequence,
        now_ms(),
        TapeEventKind::TerminalCommand,
        EventSource::Shell,
        json!({
            "phase": "started",
            "command": command.text,
            "args": args,
            "cwd": options.workspace,
        }),
        RedactionState::Redacted,
    )?;

    let request = PtyRequest {
        command: options.command,
        args: options.args,
        workspace: options.workspace,
        output_limit: options.output_limit,
        terminal_size,
    };
    let run = tokio::task::spawn_blocking(move || adapter.run(request, cancel))
        .await
        .map_err(|error| CaptureError::Worker(error.to_string()))??;
    finish_capture(&store, &mut sequence, started_at, run, redaction_config)
}

fn finish_capture(
    store: &TapeStore,
    sequence: &mut u64,
    started_at: u64,
    run: PtyRunResult,
    redaction_config: RedactionConfig,
) -> Result<CaptureSummary, CaptureError> {
    let output = String::from_utf8_lossy(&run.output);
    let redacted = redact_text(&output, &redaction_config);
    let output_truncated = run.output_truncated || redacted.truncated;
    let redaction = if redacted.redactions.is_empty() && !output_truncated {
        RedactionState::Unredacted
    } else if output_truncated {
        RedactionState::PartiallyRedacted
    } else {
        RedactionState::Redacted
    };
    append_event(
        store,
        sequence,
        now_ms(),
        TapeEventKind::TerminalCommand,
        EventSource::Shell,
        json!({
            "phase": "output",
            "stream": "pty",
            "stdout_stderr_merged": true,
            "text": redacted.text,
            "original_bytes": run.output_bytes,
            "truncated": output_truncated,
            "redactions": redacted.redactions.into_iter().map(|item| json!({
                "name": item.name,
                "original_bytes": item.original_bytes,
                "sha256": item.sha256,
            })).collect::<Vec<_>>(),
        }),
        redaction,
    )?;
    let finished_at = now_ms();
    append_event(
        store,
        sequence,
        finished_at,
        TapeEventKind::SessionFinished,
        EventSource::System,
        json!({
            "exit_code": run.exit_code,
            "signal": run.signal,
            "cancelled": run.cancelled,
            "duration_ms": finished_at.saturating_sub(started_at),
            "terminal_size": terminal_size_json(run.terminal_size),
        }),
        RedactionState::Unredacted,
    )?;
    store.finish(finished_at)?;

    Ok(CaptureSummary {
        exit_code: run.exit_code,
        signal: run.signal,
        cancelled: run.cancelled,
        output_bytes: run.output_bytes,
        output_truncated,
        terminal_size: run.terminal_size,
    })
}

fn append_event(
    store: &TapeStore,
    sequence: &mut u64,
    occurred_at_ms: u64,
    kind: TapeEventKind,
    source: EventSource,
    payload: Value,
    redaction: RedactionState,
) -> Result<(), CaptureError> {
    store.append(&TapeEvent {
        sequence: *sequence,
        occurred_at_ms,
        kind,
        source,
        payload,
        redaction,
    })?;
    *sequence += 1;
    Ok(())
}

fn terminal_size_json(size: TerminalSize) -> Value {
    json!({
        "rows": size.rows,
        "cols": size.cols,
        "pixel_width": size.pixel_width,
        "pixel_height": size.pixel_height,
    })
}

fn validate_options(options: &CaptureOptions) -> Result<(), CaptureError> {
    if options.command.is_empty() {
        return Err(CaptureError::EmptyCommand);
    }
    if !options.workspace.is_dir() {
        return Err(CaptureError::InvalidWorkspace(options.workspace.clone()));
    }
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct FakePty;

    impl PtyAdapter for FakePty {
        fn run(
            &self,
            request: PtyRequest,
            cancel: CancellationToken,
        ) -> Result<PtyRunResult, CaptureError> {
            Ok(PtyRunResult {
                exit_code: 130,
                signal: Some("SIGINT".to_owned()),
                output: b"api_key=raw-fake-secret\n".to_vec(),
                output_bytes: 24,
                output_truncated: false,
                cancelled: cancel.is_cancelled(),
                terminal_size: request.terminal_size,
            })
        }
    }

    #[tokio::test]
    async fn fake_adapter_drives_redacted_cancelled_capture_core() {
        let temp = TempDir::new().expect("temp dir");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir(&workspace).expect("workspace");
        let tape_root = temp.path().join("tape");
        let store = TapeStore::create(
            &tape_root,
            skilltape_tape::TapeManifest {
                schema: skilltape_tape::TAPE_SCHEMA_V1.to_owned(),
                id: "fake".to_owned(),
                started_at_ms: 1,
                finished_at_ms: None,
                platform: std::env::consts::OS.to_owned(),
                workspace_root: "workspace".to_owned(),
                event_count: 0,
            },
        )
        .expect("store");
        let cancel = CancellationToken::new();
        cancel.cancel();

        let summary = capture_terminal_with_adapter(
            CaptureOptions {
                command: "fake".to_owned(),
                args: vec![],
                workspace,
                env_allowlist: vec![],
                output_limit: 1024,
            },
            store,
            cancel,
            FakePty,
        )
        .await
        .expect("capture");

        assert!(summary.cancelled);
        assert_eq!(summary.exit_code, 130);
        let persisted = std::fs::read_to_string(tape_root.join("events.jsonl")).expect("events");
        assert!(!persisted.contains("raw-fake-secret"));
        assert!(persisted.contains("[REDACTED"));
    }
}
