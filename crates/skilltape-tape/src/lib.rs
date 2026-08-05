mod event;
mod session;

pub use event::{EventSource, RedactionState, TapeEvent, TapeEventKind};
pub use session::TapeManifest;

pub const TAPE_SCHEMA_V1: &str = "skilltape.dev/tape/v1";
