//! Bounded metadata-only coverage over a completely validated capture stream.
use std::collections::BTreeMap;

use serde::Serialize;
use skilltape_tape::{
    EventSource, RedactionState, TapeEvent, TapeEventKind, TapeManifest, TapeStoreError,
};

const SAMPLE_LIMIT: usize = 32;

#[derive(Debug, Serialize)]
pub struct RedactionCounts {
    pub unredacted: u64,
    pub redacted: u64,
    pub partially_redacted: u64,
}

#[derive(Debug, Serialize)]
pub struct CoverageSample {
    pub sequence: u64,
    pub redaction: &'static str,
    pub capture_warning: bool,
}

#[derive(Debug, Serialize)]
pub struct CoverageLimits {
    pub max_bytes: u64,
    pub max_events: u64,
    pub sample_limit: usize,
}

#[derive(Debug, Serialize)]
pub struct TapeCoverage {
    pub schema: &'static str,
    pub tape_id: String,
    pub state: &'static str,
    pub total_events: u64,
    pub redaction: RedactionCounts,
    pub by_kind: BTreeMap<&'static str, u64>,
    pub by_source: BTreeMap<&'static str, u64>,
    pub capture_warnings: u64,
    pub first_event_at_ms: Option<u64>,
    pub last_event_at_ms: Option<u64>,
    pub samples: Vec<CoverageSample>,
    pub sample_total: u64,
    pub samples_truncated: bool,
    pub limits: CoverageLimits,
}

pub(crate) fn summarize(
    id: &str,
    manifest: &TapeManifest,
    events: impl IntoIterator<Item = Result<TapeEvent, TapeStoreError>>,
    max_bytes: u64,
    max_events: u64,
) -> Result<TapeCoverage, TapeStoreError> {
    let mut result = TapeCoverage {
        schema: "skilltape.dev/tape-coverage/v1",
        tape_id: id.to_owned(),
        state: if manifest.finished_at_ms.is_some() {
            "complete"
        } else {
            "open"
        },
        total_events: 0,
        redaction: RedactionCounts {
            unredacted: 0,
            redacted: 0,
            partially_redacted: 0,
        },
        by_kind: BTreeMap::new(),
        by_source: BTreeMap::new(),
        capture_warnings: 0,
        first_event_at_ms: None,
        last_event_at_ms: None,
        samples: Vec::new(),
        sample_total: 0,
        samples_truncated: false,
        limits: CoverageLimits {
            max_bytes,
            max_events,
            sample_limit: SAMPLE_LIMIT,
        },
    };
    for event in events {
        let event = event?;
        let kind = match &event.kind {
            TapeEventKind::SessionStarted => "session_started",
            TapeEventKind::SessionFinished => "session_finished",
            TapeEventKind::TerminalCommand => "terminal_command",
            TapeEventKind::FilesystemChanged => "filesystem_changed",
            TapeEventKind::PermissionRequested => "permission_requested",
            TapeEventKind::PermissionDecided => "permission_decided",
            TapeEventKind::EnvironmentSnapshot => "environment_snapshot",
            TapeEventKind::CaptureWarning => "capture_warning",
        };
        let source = match &event.source {
            EventSource::Cli => "cli",
            EventSource::Shell => "shell",
            EventSource::Filesystem => "filesystem",
            EventSource::Permission => "permission",
            EventSource::Environment => "environment",
            EventSource::System => "system",
        };
        let redaction = match &event.redaction {
            RedactionState::Unredacted => {
                result.redaction.unredacted += 1;
                "unredacted"
            }
            RedactionState::Redacted => {
                result.redaction.redacted += 1;
                "redacted"
            }
            RedactionState::PartiallyRedacted => {
                result.redaction.partially_redacted += 1;
                "partially_redacted"
            }
        };
        let warning = matches!(&event.kind, TapeEventKind::CaptureWarning);
        result.capture_warnings += u64::from(warning);
        *result.by_kind.entry(kind).or_default() += 1;
        *result.by_source.entry(source).or_default() += 1;
        result.first_event_at_ms.get_or_insert(event.occurred_at_ms);
        result.last_event_at_ms = Some(event.occurred_at_ms);
        result.total_events += 1;
        if redaction != "redacted" || warning {
            result.sample_total += 1;
            if result.samples.len() < SAMPLE_LIMIT {
                result.samples.push(CoverageSample {
                    sequence: event.sequence,
                    redaction,
                    capture_warning: warning,
                });
            }
        }
    }
    result.samples_truncated = result.sample_total > result.samples.len() as u64;
    Ok(result)
}
