//! Content-free metadata for an explicitly recorded single verification attempt.
//! Local records bind each other, not authorship, saved output or current execution.
use crate::ReceiptStatus;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_VERIFY_ATTEMPTS: usize = 256;
pub const MAX_VERIFY_ATTEMPT_BYTES: u64 = 4096;
pub const MAX_VERIFY_ATTEMPT_ENTRIES: usize = 2 * MAX_VERIFY_ATTEMPTS + 2;
pub const START_SCHEMA: &str = "skilltape.dev/verify-attempt-start/v1";
pub const FINISH_SCHEMA: &str = "skilltape.dev/verify-attempt-finish/v1";
const MAX_TIME_MS: u64 = 253_402_300_799_999;

pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn valid_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyAttemptStart {
    pub schema: String,
    pub id: String,
    pub started_at_ms: u64,
}
impl VerifyAttemptStart {
    pub fn valid(&self) -> bool {
        self.schema == START_SCHEMA
            && valid_id(&self.id)
            && self.started_at_ms > 0
            && self.started_at_ms <= MAX_TIME_MS
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifyAttemptOutcome {
    ReceiptProduced,
    PreparationFailed,
    Unconfirmed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyAttemptReceipt {
    pub receipt_sha256: String,
    pub run_id: String,
    pub skill_hash: String,
    pub input_set_sha256: String,
    pub status: ReceiptStatus,
}
impl VerifyAttemptReceipt {
    fn valid(&self) -> bool {
        [
            &self.receipt_sha256,
            &self.run_id,
            &self.skill_hash,
            &self.input_set_sha256,
        ]
        .into_iter()
        .all(|v| valid_id(v))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyAttemptFinish {
    pub schema: String,
    pub id: String,
    pub start_sha256: String,
    pub finished_at_ms: u64,
    pub outcome: VerifyAttemptOutcome,
    pub receipt: Option<VerifyAttemptReceipt>,
}
impl VerifyAttemptFinish {
    pub fn matches_start(&self, start: &VerifyAttemptStart, bytes: &[u8]) -> bool {
        start.valid()
            && serde_json::from_slice::<VerifyAttemptStart>(bytes)
                .ok()
                .as_ref()
                == Some(start)
            && self.schema == FINISH_SCHEMA
            && self.id == start.id
            && self.start_sha256 == digest(bytes)
            && self.finished_at_ms >= start.started_at_ms
            && self.finished_at_ms <= MAX_TIME_MS
            && match (self.outcome, &self.receipt) {
                (VerifyAttemptOutcome::ReceiptProduced, Some(receipt)) => receipt.valid(),
                (
                    VerifyAttemptOutcome::PreparationFailed | VerifyAttemptOutcome::Unconfirmed,
                    None,
                ) => true,
                _ => false,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_is_bound_to_immutable_start_and_canonical_status() {
        let start = VerifyAttemptStart {
            schema: START_SCHEMA.into(),
            id: "a".repeat(64),
            started_at_ms: 1,
        };
        let raw = serde_json::to_vec(&start).unwrap();
        let mut finish = VerifyAttemptFinish {
            schema: FINISH_SCHEMA.into(),
            id: start.id.clone(),
            start_sha256: digest(&raw),
            finished_at_ms: 2,
            outcome: VerifyAttemptOutcome::Unconfirmed,
            receipt: None,
        };
        assert!(finish.matches_start(&start, &raw));
        assert!(!finish.matches_start(&start, b"{}"));
        finish.outcome = VerifyAttemptOutcome::ReceiptProduced;
        assert!(!finish.matches_start(&start, &raw));
        finish.receipt = Some(VerifyAttemptReceipt {
            receipt_sha256: "b".repeat(64),
            run_id: "e".repeat(64),
            skill_hash: "c".repeat(64),
            input_set_sha256: "d".repeat(64),
            status: ReceiptStatus::Cancelled,
        });
        assert!(finish.matches_start(&start, &raw));
        let mut value = serde_json::to_value(&finish).unwrap();
        value["receipt"]["status"] = serde_json::json!("policy_denied");
        assert!(serde_json::from_value::<VerifyAttemptFinish>(value).is_err());
        finish.finished_at_ms = 0;
        assert!(!finish.matches_start(&start, &raw));
    }
}
