use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{CompileError, CompileOutput};

/// The deterministic artifact and its content hash supplied to an optional provider.
///
/// This boundary intentionally contains no tape events, provider client, or filesystem
/// handle. Providers receive only the already-compiled metadata artifact.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalInput {
    pub base: CompileOutput,
    pub input_hash: String,
}

impl ProposalInput {
    pub fn from_base(base: &CompileOutput) -> Result<Self, CompileError> {
        Ok(Self {
            base: base.clone(),
            input_hash: base.content_hash()?,
        })
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    #[default]
    Pending,
    Accepted,
    Rejected,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowProposal {
    pub workflow_patch: serde_json::Value,
    pub descriptions: BTreeMap<String, String>,
    pub model: String,
    pub input_hash: String,
    #[serde(default)]
    pub status: ProposalStatus,
}

impl WorkflowProposal {
    pub fn pending(
        workflow_patch: serde_json::Value,
        descriptions: BTreeMap<String, String>,
        model: impl Into<String>,
        input_hash: impl Into<String>,
    ) -> Self {
        Self {
            workflow_patch,
            descriptions,
            model: model.into(),
            input_hash: input_hash.into(),
            status: ProposalStatus::Pending,
        }
    }
}

/// Provider failures are data-only and have no transport or SDK dependency.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProviderError {
    #[error("provider is offline")]
    Offline,
    #[error("provider request timed out")]
    Timeout,
    #[error("provider returned invalid JSON")]
    InvalidJson,
    #[error("provider quota was exceeded")]
    Quota,
    #[error("provider failed: {message}")]
    Failed { message: String },
}

/// Optional proposal boundary. Deterministic compilation never calls this trait.
#[allow(async_fn_in_trait)]
pub trait ProposalProvider {
    async fn propose(&self, input: ProposalInput) -> Result<WorkflowProposal, ProviderError>;
}
