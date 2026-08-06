use serde::{Deserialize, Serialize};

use crate::{CompileError, CompileTarget};

pub const COMPILE_SCHEMA_V1: &str = "skilltape.dev/compile/v1";

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
}

impl CompileProvenance {
    pub fn new(target: CompileTarget, steps: Vec<StepProvenance>) -> Self {
        Self {
            schema: COMPILE_SCHEMA_V1.into(),
            target_identity: target.identity(),
            steps,
        }
    }
}
