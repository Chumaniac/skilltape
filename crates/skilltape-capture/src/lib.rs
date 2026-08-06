//! Secret-safe primitives shared by Skilltape capture backends.

mod environment;
mod redaction;

pub use environment::{snapshot_environment, EnvironmentSnapshot, EnvironmentVariable};
pub use redaction::{redact_text, RedactedText, RedactionConfig, RedactionMetadata};
