use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TapeEventKind {
    SessionStarted,
    SessionFinished,
    TerminalCommand,
    FilesystemChanged,
    PermissionRequested,
    PermissionDecided,
    EnvironmentSnapshot,
    CaptureWarning,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    Cli,
    Shell,
    Filesystem,
    Permission,
    Environment,
    System,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactionState {
    Unredacted,
    Redacted,
    PartiallyRedacted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TapeEvent {
    pub sequence: u64,
    pub occurred_at_ms: u64,
    pub kind: TapeEventKind,
    pub source: EventSource,
    pub payload: serde_json::Value,
    pub redaction: RedactionState,
}
