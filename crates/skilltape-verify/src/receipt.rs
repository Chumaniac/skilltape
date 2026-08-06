use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use skilltape_runner::{PolicyDecisionRecord, RunEvent, RunStatus, RunSummary, StepStatus};
use thiserror::Error;

pub const RECEIPT_SCHEMA_V1: &str = "skilltape.dev/receipt/v1";
pub const RUN_SCHEMA_V1: &str = "skilltape.dev/run/v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptStatus {
    Succeeded,
    RunFailed,
    Cancelled,
    AssertionFailed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReceiptStep {
    pub step_id: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub stdout_sha256: String,
    pub stdout_bytes: usize,
    pub stdout_truncated: bool,
    pub stderr_sha256: String,
    pub stderr_bytes: usize,
    pub stderr_truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PolicyDecisionSummary {
    pub step_id: String,
    pub phase: String,
    pub allowed: bool,
    pub code: String,
    pub reason: String,
    pub risk: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Receipt {
    pub schema: String,
    pub run_id: String,
    pub skill_hash: String,
    pub status: ReceiptStatus,
    pub steps: Vec<ReceiptStep>,
    pub assertions: Vec<super::AssertionResult>,
    pub policy_decisions: Vec<PolicyDecisionSummary>,
}

#[derive(Debug, Error)]
pub(crate) enum ReceiptError {
    #[error("receipt schema validation failed: {message}")]
    Schema { message: String },
    #[error("receipt serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub(crate) fn build(
    run_id: String,
    skill_hash: String,
    summary: &RunSummary,
    assertions: Vec<super::AssertionResult>,
) -> Receipt {
    let status = match summary.status {
        RunStatus::Failed => ReceiptStatus::RunFailed,
        RunStatus::Cancelled => ReceiptStatus::Cancelled,
        RunStatus::Succeeded if assertions.iter().any(|assertion| !assertion.passed) => {
            ReceiptStatus::AssertionFailed
        }
        RunStatus::Succeeded => ReceiptStatus::Succeeded,
    };
    Receipt {
        schema: RECEIPT_SCHEMA_V1.into(),
        run_id,
        skill_hash,
        status,
        steps: summary
            .steps
            .iter()
            .map(|step| ReceiptStep {
                step_id: step.step_id.clone(),
                status: step_status(step.status).into(),
                exit_code: step.exit_code,
                stdout_sha256: digest(&step.stdout),
                stdout_bytes: step.stdout.len(),
                stdout_truncated: step.stdout_truncated,
                stderr_sha256: digest(&step.stderr),
                stderr_bytes: step.stderr.len(),
                stderr_truncated: step.stderr_truncated,
            })
            .collect(),
        assertions,
        policy_decisions: summary
            .policy_decisions
            .iter()
            .map(policy_decision)
            .collect(),
    }
}

pub(crate) fn validate_receipt(receipt: &Receipt) -> Result<(), ReceiptError> {
    let value = serde_json::to_value(receipt)?;
    validate_schema(include_str!("../../../schemas/receipt/v1.json"), &value)
}

pub(crate) fn run_event_document(event: &RunEvent) -> Value {
    serde_json::json!({
        "schema": RUN_SCHEMA_V1,
        "sequence": event.sequence,
        "step_id": event.step_id,
        "status": step_status(event.status),
        "stdout_sha256": digest(&event.stdout),
        "stdout_bytes": event.stdout.len(),
        "stderr_sha256": digest(&event.stderr),
        "stderr_bytes": event.stderr.len(),
        "exit_code": event.exit_code,
    })
}

pub(crate) fn validate_run_event(event: &RunEvent) -> Result<(), ReceiptError> {
    validate_schema(
        include_str!("../../../schemas/run/v1.json"),
        &run_event_document(event),
    )
}

fn validate_schema(schema: &str, value: &Value) -> Result<(), ReceiptError> {
    let schema: Value = serde_json::from_str(schema)?;
    let validator = jsonschema::validator_for(&schema).map_err(|error| ReceiptError::Schema {
        message: error.to_string(),
    })?;
    let first_error = validator.iter_errors(value).next();
    first_error.map_or(Ok(()), |error| {
        Err(ReceiptError::Schema {
            message: error.to_string(),
        })
    })
}

fn policy_decision(decision: &PolicyDecisionRecord) -> PolicyDecisionSummary {
    PolicyDecisionSummary {
        step_id: decision.step_id.clone(),
        phase: match decision.phase {
            skilltape_runner::PolicyPhase::Before => "before",
            skilltape_runner::PolicyPhase::After => "after",
        }
        .into(),
        allowed: decision.decision.allowed,
        code: decision.decision.code.clone(),
        reason: decision.decision.reason.clone(),
        risk: decision.decision.risk.as_str().into(),
    }
}

fn step_status(status: StepStatus) -> &'static str {
    match status {
        StepStatus::RunStarted => "run_started",
        StepStatus::Started => "started",
        StepStatus::Succeeded => "succeeded",
        StepStatus::Failed => "failed",
        StepStatus::Denied => "denied",
        StepStatus::TimedOut => "timed_out",
        StepStatus::Cancelled => "cancelled",
        StepStatus::SpawnFailed => "spawn_failed",
        StepStatus::RunSucceeded => "run_succeeded",
        StepStatus::RunFailed => "run_failed",
        StepStatus::RunCancelled => "run_cancelled",
    }
}

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
