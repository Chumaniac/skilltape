use std::collections::BTreeSet;

use serde::Serialize;
use skilltape_core::LoadedSkillPackage;
use skilltape_policy::{PolicyDecision, PolicyEngine};

use crate::VerifyError;

#[derive(Clone, Debug, Serialize)]
pub struct PolicyPlan {
    pub schema: &'static str,
    pub execution: &'static str,
    pub allowed: bool,
    pub package_valid: bool,
    pub package_error_codes: BTreeSet<String>,
    pub checks: Vec<PolicyPlanCheck>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PolicyPlanCheck {
    pub step_id: String,
    pub ordinal: usize,
    #[serde(flatten)]
    pub decision: PolicyDecision,
}

/// Read-only package lint plus pure policy decisions. No input is opened, no
/// process is spawned and no replay workspace, receipt or output is created.
pub fn policy_plan(package: &LoadedSkillPackage) -> Result<PolicyPlan, VerifyError> {
    let records = skilltape_runner::preview_policy(package, &PolicyEngine::default())?;
    let lint = package.lint(false);
    let package_valid = lint.errors.is_empty();
    Ok(PolicyPlan {
        schema: "skilltape.dev/policy-plan/v1",
        execution: "not-run",
        allowed: package_valid && records.iter().all(|item| item.decision.allowed),
        package_valid,
        package_error_codes: lint.errors.into_iter().map(|item| item.code).collect(),
        checks: records
            .into_iter()
            .enumerate()
            .map(|(index, item)| PolicyPlanCheck {
                step_id: item.step_id,
                ordinal: index + 1,
                decision: item.decision,
            })
            .collect(),
    })
}
