use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use skilltape_runner::ResourceLimits;

use crate::{Receipt, VerifyError};

/// Optional metadata beside a receipt, never a modification of Receipt v1.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunContext {
    pub schema: String,
    pub receipt_sha256: String,
    pub run_id: String,
    pub skill_hash: String,
    pub input_set_sha256: String,
    pub input_observation: String,
    pub executor: String,
    pub executor_version: String,
    pub os: String,
    pub arch: String,
    pub max_processes: u32,
    pub step_timeout_ms: u64,
    pub max_output_bytes: usize,
}

impl RunContext {
    pub fn new(
        receipt: &Receipt,
        input_hash: String,
        limits: &ResourceLimits,
    ) -> Result<Self, VerifyError> {
        Ok(Self {
            schema: "skilltape.dev/run-context/v1".into(),
            receipt_sha256: sha256(&serde_json::to_vec(receipt)?),
            run_id: receipt.run_id.clone(),
            skill_hash: receipt.skill_hash.clone(),
            input_set_sha256: input_hash,
            // Observed by the existing bounded pre-run scan; not a frozen-input or author proof.
            input_observation: "bounded-before-run".into(),
            executor: "skilltape-runner/sequential-local-v1".into(),
            executor_version: skilltape_runner::RUNNER_VERSION.into(),
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            max_processes: limits.max_processes,
            step_timeout_ms: limits
                .step_timeout
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            max_output_bytes: limits.max_output_bytes,
        })
    }

    /// Integrity binding only. A locally written sidecar cannot authenticate execution.
    pub fn binds_receipt(&self, receipt: &[u8]) -> bool {
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(receipt) else {
            return false;
        };
        if crate::receipt::validate_receipt_value(&value).is_err() {
            return false;
        }
        let digest = |v: &str| {
            v.len() == 64
                && v.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        let identifier = |v: &str| {
            !v.is_empty()
                && v.len() <= 64
                && v.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
        };
        self.schema == "skilltape.dev/run-context/v1"
            && value["schema"] == "skilltape.dev/receipt/v1"
            && value["run_id"].as_str() == Some(self.run_id.as_str())
            && value["skill_hash"].as_str() == Some(self.skill_hash.as_str())
            && self.receipt_sha256 == sha256(receipt)
            && [
                &self.receipt_sha256,
                &self.run_id,
                &self.skill_hash,
                &self.input_set_sha256,
            ]
            .iter()
            .all(|s| digest(s))
            && self.input_observation == "bounded-before-run"
            && self.executor == "skilltape-runner/sequential-local-v1"
            && [&self.executor_version, &self.os, &self.arch]
                .iter()
                .all(|s| identifier(s))
            && self.max_processes > 0
            && self.step_timeout_ms > 0
            && self.max_output_bytes > 0
    }
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
