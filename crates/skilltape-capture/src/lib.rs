//! Secret-safe primitives shared by Skilltape capture backends.

mod environment;
mod pty;
mod redaction;
mod session;

pub use environment::{snapshot_environment, EnvironmentSnapshot, EnvironmentVariable};
pub use redaction::{redact_text, RedactedText, RedactionConfig, RedactionMetadata};
pub use session::{capture_terminal, CaptureError, CaptureOptions, CaptureSummary, TerminalSize};
