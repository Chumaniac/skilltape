use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use skilltape_tape::TapeManifest;

use crate::{CompileError, CompileTarget, TapeSession};

pub const COMPILE_SCHEMA_V1: &str = "skilltape.dev/compile/v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceTape {
    pub format: String,
    pub manifest_sha256: String,
    pub events_sha256: String,
    pub event_count: u64,
}

impl SourceTape {
    /// Digests the normalized v1 values consumed by the compiler, not the
    /// original file formatting. No workspace paths or payloads are copied.
    pub fn new(manifest: &TapeManifest, tape: &TapeSession) -> Result<Self, serde_json::Error> {
        fn digest(value: &impl Serialize) -> Result<String, serde_json::Error> {
            let bytes = serde_json::to_vec(&serde_json::to_value(value)?)?;
            Ok(format!(
                "sha256:{}",
                Sha256::digest(bytes)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            ))
        }
        Ok(Self {
            format: "skilltape.dev/tape-source/v1".into(),
            manifest_sha256: digest(manifest)?,
            events_sha256: digest(&tape.events())?,
            event_count: tape.events().len() as u64,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StepProvenance {
    pub step_id: String,
    pub event_sequences: Vec<u64>,
    pub source_summary: String,
}

impl StepProvenance {
    pub fn new(
        step_id: impl Into<String>,
        event_sequences: Vec<u64>,
        source_summary: impl Into<String>,
    ) -> Result<Self, CompileError> {
        let source = Self {
            step_id: step_id.into(),
            event_sequences,
            source_summary: source_summary.into(),
        };
        source.validate()?;
        Ok(source)
    }

    pub(crate) fn validate(&self) -> Result<(), CompileError> {
        if self.event_sequences.is_empty() {
            return Err(CompileError::MissingSource {
                step_id: self.step_id.clone(),
            });
        }
        for pair in self.event_sequences.windows(2) {
            match pair {
                [previous, next] if previous == next => {
                    return Err(CompileError::DuplicateSource {
                        step_id: self.step_id.clone(),
                        event_sequence: *next,
                    })
                }
                [previous, next] if previous > next => {
                    return Err(CompileError::OutOfOrderSource {
                        step_id: self.step_id.clone(),
                        previous: *previous,
                        next: *next,
                    })
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompileProvenance {
    pub schema: String,
    pub target_identity: String,
    pub steps: Vec<StepProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_tape: Option<SourceTape>,
}

impl CompileProvenance {
    pub fn new(target: CompileTarget, steps: Vec<StepProvenance>) -> Self {
        Self {
            schema: COMPILE_SCHEMA_V1.into(),
            target_identity: target.identity(),
            steps,
            source_tape: None,
        }
    }
}
