//! Bounded metadata shared by the batch CLI and the read-only Console.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const BATCH_PLAN_SCHEMA: &str = "skilltape.dev/batch-plan/v1";
pub const BATCH_STATE_SCHEMA: &str = "skilltape.dev/batch-state/v1";
pub const BATCH_REVIEW_SCHEMA: &str = "skilltape.dev/batch-review/v1";
// Serial first release: larger queues require explicit storage/retention design.
pub const MAX_BATCH_CASES: usize = 16;
pub const MAX_BATCH_ATTEMPTS: usize = 3;
pub const MAX_BATCH_METADATA_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchPlan {
    pub schema: String,
    pub skill: String,
    pub cases: Vec<BatchCase>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchCase {
    pub id: String,
    pub input: String,
    #[serde(default)]
    pub expected_files: Vec<String>,
    #[serde(default)]
    pub expected_hashes: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Unconfirmed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchAttempt {
    pub number: u8,
    pub status: BatchStatus,
    pub input_sha256: String,
    pub generation: u8,
    pub receipt_sha256: Option<String>,
    pub context_sha256: Option<String>,
    pub delivery_manifest_sha256: Option<String>,
    pub artifact_tree_sha256: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchCaseState {
    pub id: String,
    pub input_sha256: String,
    pub generation: u8,
    pub attempts: Vec<BatchAttempt>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchState {
    pub schema: String,
    pub plan_sha256: String,
    pub package_sha256: String,
    pub revision: u64,
    pub cases: Vec<BatchCaseState>,
}

#[derive(Debug, Serialize)]
pub struct BatchReview {
    pub schema: &'static str,
    pub revision: u64,
    pub status: &'static str,
    pub package_sha256: String,
    pub plan_sha256: String,
    pub cases: Vec<BatchCaseReview>,
    pub max_cases: usize,
    pub max_attempts: usize,
    pub provenance: &'static str,
}

#[derive(Debug, Serialize)]
pub struct BatchCaseReview {
    pub id: String,
    pub status: BatchStatus,
    pub attempt_count: usize,
    pub input_sha256: String,
    pub receipt_id: Option<String>,
    pub delivery_id: Option<String>,
    pub evidence: &'static str,
    pub error_code: Option<String>,
    pub attempts: Vec<BatchAttemptReview>,
}

#[derive(Debug, Serialize)]
pub struct BatchAttemptReview {
    pub number: u8,
    pub status: BatchStatus,
    pub input_sha256: String,
    pub receipt_id: Option<String>,
    pub delivery_id: Option<String>,
}

pub fn batch_case_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 48
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

pub fn batch_sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn relative(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1024
        && !value.contains(['\\', ':'])
        && !value.chars().any(char::is_control)
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

impl BatchPlan {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != BATCH_PLAN_SCHEMA
            || !relative(&self.skill)
            || self.cases.is_empty()
            || self.cases.len() > MAX_BATCH_CASES
        {
            return Err("invalid batch plan or case capacity");
        }
        let mut ids = BTreeSet::new();
        for case in &self.cases {
            if !batch_case_id(&case.id)
                || !ids.insert(&case.id)
                || !relative(&case.input)
                || case.expected_files.len() + case.expected_hashes.len() > 64
                || case.expected_files.iter().any(|path| !relative(path))
            {
                return Err("invalid case identity, input path or output requirements");
            }
            let mut hashes = BTreeMap::new();
            for requirement in &case.expected_hashes {
                let Some((path, digest)) = requirement.rsplit_once('=') else {
                    return Err("invalid output digest requirement");
                };
                if !relative(path)
                    || digest.len() != 64
                    || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                {
                    return Err("invalid output digest requirement");
                }
                let normalized = path.strip_prefix("outputs/").unwrap_or(path);
                if hashes
                    .insert(normalized, digest.to_ascii_lowercase())
                    .is_some_and(|previous| previous != digest.to_ascii_lowercase())
                {
                    return Err("conflicting output digest requirements");
                }
            }
        }
        Ok(())
    }
}

impl BatchState {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != BATCH_STATE_SCHEMA
            || self.revision == 0
            || !batch_sha(&self.plan_sha256)
            || !batch_sha(&self.package_sha256)
            || self.cases.is_empty()
            || self.cases.len() > MAX_BATCH_CASES
        {
            return Err("invalid batch state");
        }
        let mut ids = BTreeSet::new();
        for case in &self.cases {
            if !batch_case_id(&case.id)
                || !ids.insert(&case.id)
                || !batch_sha(&case.input_sha256)
                || case.generation as usize > MAX_BATCH_ATTEMPTS
                || case.attempts.len() > MAX_BATCH_ATTEMPTS
            {
                return Err("invalid batch case state");
            }
            for (index, attempt) in case.attempts.iter().enumerate() {
                if attempt.number as usize != index + 1
                    || attempt.generation as usize > MAX_BATCH_ATTEMPTS
                    || !batch_sha(&attempt.input_sha256)
                    || attempt.status == BatchStatus::Pending
                    || [
                        &attempt.receipt_sha256,
                        &attempt.context_sha256,
                        &attempt.delivery_manifest_sha256,
                        &attempt.artifact_tree_sha256,
                    ]
                    .iter()
                    .any(|value| value.as_ref().is_some_and(|digest| !batch_sha(digest)))
                    || attempt.error_code.as_deref().is_some_and(|code| {
                        !["execution-unconfirmed", "evidence-changed"].contains(&code)
                    })
                {
                    return Err("invalid batch attempt");
                }
                if attempt.status == BatchStatus::Succeeded
                    && [
                        &attempt.receipt_sha256,
                        &attempt.context_sha256,
                        &attempt.delivery_manifest_sha256,
                        &attempt.artifact_tree_sha256,
                    ]
                    .iter()
                    .any(|value| value.is_none())
                {
                    return Err("successful attempt has no bound evidence");
                }
                if matches!(attempt.status, BatchStatus::Failed | BatchStatus::Cancelled)
                    && (attempt.receipt_sha256.is_none() || attempt.context_sha256.is_none())
                {
                    return Err("terminal attempt has no Receipt");
                }
                if index + 1 < case.attempts.len()
                    && !matches!(attempt.status, BatchStatus::Failed | BatchStatus::Cancelled)
                {
                    return Err("successful or unknown attempts cannot be retried");
                }
            }
            if case.attempts.last().is_some_and(|last| {
                last.input_sha256 != case.input_sha256 || last.generation != case.generation
            }) {
                return Err("current input does not bind the latest attempt");
            }
        }
        Ok(())
    }

    pub fn review(&self) -> BatchReview {
        let cases: Vec<_> = self
            .cases
            .iter()
            .map(|case| {
                let last = case.attempts.last();
                let status = last.map_or(BatchStatus::Pending, |attempt| attempt.status);
                let id = last.map(|attempt| format!("{}-a{}", case.id, attempt.number));
                BatchCaseReview {
                    id: case.id.clone(),
                    status,
                    attempt_count: case.attempts.len(),
                    input_sha256: case.input_sha256.clone(),
                    receipt_id: last
                        .and_then(|attempt| attempt.receipt_sha256.as_ref())
                        .and(id.clone()),
                    delivery_id: (status == BatchStatus::Succeeded).then_some(id).flatten(),
                    evidence: if last.is_some_and(|attempt| attempt.receipt_sha256.is_some()) {
                        "recorded"
                    } else {
                        "unrecorded"
                    },
                    error_code: last.and_then(|attempt| attempt.error_code.clone()),
                    attempts: case
                        .attempts
                        .iter()
                        .map(|attempt| {
                            let id = format!("{}-a{}", case.id, attempt.number);
                            BatchAttemptReview {
                                number: attempt.number,
                                status: attempt.status,
                                input_sha256: attempt.input_sha256.clone(),
                                receipt_id: attempt.receipt_sha256.as_ref().map(|_| id.clone()),
                                delivery_id: (attempt.status == BatchStatus::Succeeded)
                                    .then_some(id),
                            }
                        })
                        .collect(),
                }
            })
            .collect();
        let status = if cases
            .iter()
            .all(|case| case.status == BatchStatus::Succeeded)
        {
            "complete"
        } else if cases
            .iter()
            .any(|case| matches!(case.status, BatchStatus::Running | BatchStatus::Unconfirmed))
        {
            "unconfirmed"
        } else if cases.iter().all(|case| case.status == BatchStatus::Pending) {
            "pending"
        } else {
            "partial"
        };
        BatchReview {
            schema: BATCH_REVIEW_SCHEMA,
            revision: self.revision,
            status,
            package_sha256: self.package_sha256.clone(),
            plan_sha256: self.plan_sha256.clone(),
            cases,
            max_cases: MAX_BATCH_CASES,
            max_attempts: MAX_BATCH_ATTEMPTS,
            provenance: "local-evidence-not-authenticated",
        }
    }
}
