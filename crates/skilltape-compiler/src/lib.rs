mod provenance;

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use skilltape_schema::{Permissions, Step, Workflow};
use skilltape_tape::TapeEvent;
use thiserror::Error;

pub use provenance::{CompileProvenance, StepProvenance, COMPILE_SCHEMA_V1};

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("tape event sequence is out of order: expected {expected}, got {actual}")]
    TapeEventOutOfOrder { expected: u64, actual: u64 },
    #[error("tape event sequence {sequence} is duplicated")]
    DuplicateTapeEvent { sequence: u64 },
    #[error("compile name must not be empty")]
    InvalidCompileName,
    #[error("compile target name must not be empty")]
    InvalidTargetName,
    #[error("compile target version must not be empty")]
    InvalidTargetVersion,
    #[error("compile target name or version contains unsupported `@`")]
    InvalidTargetIdentity,
    #[error("workflow contains duplicate step id `{step_id}`")]
    DuplicateWorkflowStep { step_id: String },
    #[error("provenance contains an unknown workflow step `{step_id}`")]
    UnknownStepProvenance { step_id: String },
    #[error("workflow step `{step_id}` has duplicate provenance")]
    DuplicateStepProvenance { step_id: String },
    #[error("workflow step `{step_id}` is missing provenance")]
    MissingSource { step_id: String },
    #[error("workflow step `{step_id}` references unknown tape event {event_sequence}")]
    UnknownSource {
        step_id: String,
        event_sequence: u64,
    },
    #[error("workflow step `{step_id}` references tape event {event_sequence} more than once")]
    DuplicateSource {
        step_id: String,
        event_sequence: u64,
    },
    #[error(
        "workflow step `{step_id}` provenance is out of order: {previous} is followed by {next}"
    )]
    OutOfOrderSource {
        step_id: String,
        previous: u64,
        next: u64,
    },
    #[error("JSON serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TapeSession {
    events: Vec<TapeEvent>,
}

#[derive(Deserialize)]
struct TapeSessionFields {
    events: Vec<TapeEvent>,
}

impl<'de> Deserialize<'de> for TapeSession {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let fields = TapeSessionFields::deserialize(deserializer)?;
        Self::new(fields.events).map_err(serde::de::Error::custom)
    }
}

impl TapeSession {
    pub fn new(events: Vec<TapeEvent>) -> Result<Self, CompileError> {
        for (expected, event) in events.iter().enumerate() {
            let expected = expected as u64;
            if event.sequence == expected {
                continue;
            }
            if event.sequence < expected {
                return Err(CompileError::DuplicateTapeEvent {
                    sequence: event.sequence,
                });
            }
            return Err(CompileError::TapeEventOutOfOrder {
                expected,
                actual: event.sequence,
            });
        }
        Ok(Self { events })
    }

    pub fn try_new(events: Vec<TapeEvent>) -> Result<Self, CompileError> {
        Self::new(events)
    }

    pub fn events(&self) -> &[TapeEvent] {
        &self.events
    }

    pub fn event(&self, sequence: u64) -> Option<&TapeEvent> {
        usize::try_from(sequence)
            .ok()
            .and_then(|index| self.events.get(index))
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompileTarget {
    pub name: String,
    pub version: String,
}

impl CompileTarget {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Result<Self, CompileError> {
        let name = name.into();
        let version = version.into();
        if name.is_empty() {
            return Err(CompileError::InvalidTargetName);
        }
        if version.is_empty() {
            return Err(CompileError::InvalidTargetVersion);
        }
        if name.contains('@') || version.contains('@') {
            return Err(CompileError::InvalidTargetIdentity);
        }
        Ok(Self { name, version })
    }

    pub fn identity(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompileRequest {
    pub tape: TapeSession,
    pub name: String,
    pub target: CompileTarget,
}

impl CompileRequest {
    pub fn new(
        tape: TapeSession,
        name: impl Into<String>,
        target: CompileTarget,
    ) -> Result<Self, CompileError> {
        let name = name.into();
        if name.is_empty() {
            return Err(CompileError::InvalidCompileName);
        }
        Ok(Self { tape, name, target })
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct FixtureDraft {
    pub files: std::collections::BTreeMap<String, String>,
}

impl FixtureDraft {
    pub fn new(files: std::collections::BTreeMap<String, String>) -> Self {
        Self { files }
    }

    pub fn empty() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CompileOutput {
    pub workflow: Workflow,
    pub permissions: Permissions,
    pub skill_markdown: String,
    pub fixtures: FixtureDraft,
    pub provenance: Vec<StepProvenance>,
}

impl CompileOutput {
    pub fn try_new(
        tape: &TapeSession,
        workflow: Workflow,
        permissions: Permissions,
        skill_markdown: String,
        fixtures: FixtureDraft,
        provenance: Vec<StepProvenance>,
    ) -> Result<Self, CompileError> {
        let provenance = canonicalize_provenance(tape, &workflow, provenance)?;
        Ok(Self {
            workflow,
            permissions,
            skill_markdown,
            fixtures,
            provenance,
        })
    }

    pub fn validate(&self, tape: &TapeSession) -> Result<(), CompileError> {
        canonicalize_provenance(tape, &self.workflow, self.provenance.clone()).map(|_| ())
    }

    pub fn provenance_document(&self, target: CompileTarget) -> CompileProvenance {
        CompileProvenance::new(target, self.provenance.clone())
    }

    pub fn deterministic_json(&self) -> Result<Vec<u8>, CompileError> {
        Ok(serde_json::to_vec(self)?)
    }

    pub fn content_hash(&self) -> Result<String, CompileError> {
        let bytes = self.deterministic_json()?;
        Ok(format!("{:x}", Sha256::digest(bytes)))
    }

    pub fn deterministic_hash(&self) -> Result<String, CompileError> {
        self.content_hash()
    }
}

pub trait Compiler {
    fn compile(&self, request: CompileRequest) -> Result<CompileOutput, CompileError>;
}

fn canonicalize_provenance(
    tape: &TapeSession,
    workflow: &Workflow,
    provenance: Vec<StepProvenance>,
) -> Result<Vec<StepProvenance>, CompileError> {
    let mut workflow_ids = Vec::with_capacity(workflow.steps.len());
    let mut seen_workflow_ids = BTreeSet::new();
    for step in &workflow.steps {
        let step_id = step_id(step).to_owned();
        if !seen_workflow_ids.insert(step_id.clone()) {
            return Err(CompileError::DuplicateWorkflowStep { step_id });
        }
        workflow_ids.push(step_id);
    }

    let mut by_step = std::collections::BTreeMap::new();
    for source in provenance {
        source.validate()?;
        let step_id = source.step_id.clone();
        if !seen_workflow_ids.contains(&step_id) {
            return Err(CompileError::UnknownStepProvenance { step_id });
        }
        if by_step.contains_key(&step_id) {
            return Err(CompileError::DuplicateStepProvenance { step_id });
        }
        by_step.insert(step_id, source);
    }

    let mut ordered = Vec::with_capacity(workflow_ids.len());
    for step_id in workflow_ids {
        let source = by_step
            .remove(&step_id)
            .ok_or_else(|| CompileError::MissingSource {
                step_id: step_id.clone(),
            })?;
        for &event_sequence in &source.event_sequences {
            if tape.event(event_sequence).is_none() {
                return Err(CompileError::UnknownSource {
                    step_id: step_id.clone(),
                    event_sequence,
                });
            }
        }
        ordered.push(source);
    }
    Ok(ordered)
}

fn step_id(step: &Step) -> &str {
    match step {
        Step::Exec(step) => &step.id,
        Step::Script(step) => &step.id,
        Step::File(step) => &step.id,
        Step::Assert(step) => &step.id,
    }
}
