//! Pure declared-capability checks shared by preflight and replay.

use skilltape_core::LoadedSkillPackage;
use skilltape_policy::{FileAccess, PolicyDecision, PolicyEngine};
use skilltape_schema::Step;

use crate::{step_id, PolicyDecisionRecord, PolicyPhase, RunError};

/// Preview the same declared-capability decisions used by replay, without any I/O.
/// This is not input, sandbox, output, or successful-execution verification.
pub fn preview_policy(
    package: &LoadedSkillPackage,
    policy: &PolicyEngine,
) -> Result<Vec<PolicyDecisionRecord>, RunError> {
    const MAX_CHECKS: usize = 1000;
    if package.workflow.steps.len() > MAX_CHECKS {
        return Err(RunError::InvalidLimits {
            message: "policy preview exceeds 1000 steps or checks".into(),
        });
    }
    let mut records = Vec::new();
    for step in &package.workflow.steps {
        let count = match step {
            Step::Exec(step) => 1 + step.outputs.len(),
            Step::Script(step) => 2 + step.outputs.len(),
            Step::File(step) => {
                if step.operation == "mkdir" {
                    1
                } else {
                    2
                }
            }
            Step::Assert(_) => 1,
        };
        if count > MAX_CHECKS - records.len() {
            return Err(RunError::InvalidLimits {
                message: "policy preview exceeds 1000 steps or checks".into(),
            });
        }
        record_decisions(
            &mut records,
            step_id(step),
            PolicyPhase::Before,
            &policy_checks(policy, step, package),
        );
    }
    Ok(records)
}

pub(super) fn policy_checks(
    policy: &PolicyEngine,
    step: &Step,
    package: &LoadedSkillPackage,
) -> Vec<PolicyDecision> {
    let mut decisions = Vec::new();
    let permissions = &package.permissions;
    match step {
        Step::Exec(step) => {
            decisions.push(policy.check_command(&step.program, &step.args, permissions));
            for output in &step.outputs {
                decisions.push(policy.check_path(&output.path, FileAccess::Write, permissions));
            }
        }
        Step::Script(step) => {
            decisions.push(policy.check_path(&step.path, FileAccess::Read, permissions));
            decisions.push(policy.check_command(&step.path, &step.args, permissions));
            for output in &step.outputs {
                decisions.push(policy.check_path(&output.path, FileAccess::Write, permissions));
            }
        }
        Step::File(step) => {
            if step.operation != "mkdir" {
                decisions.push(policy.check_path(&step.from_path, FileAccess::Read, permissions));
            }
            decisions.push(policy.check_path(&step.to_path, FileAccess::Write, permissions));
        }
        Step::Assert(step) => {
            decisions.push(policy.check_path(
                step.assertion.path.as_deref().unwrap_or_default(),
                FileAccess::Read,
                permissions,
            ));
        }
    }
    decisions
}

pub(super) fn record_decisions(
    records: &mut Vec<PolicyDecisionRecord>,
    step_id: &str,
    phase: PolicyPhase,
    decisions: &[PolicyDecision],
) {
    records.extend(
        decisions
            .iter()
            .cloned()
            .map(|decision| PolicyDecisionRecord {
                step_id: step_id.to_owned(),
                phase,
                decision,
            }),
    );
}
