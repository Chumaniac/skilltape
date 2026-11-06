//! Recorded outcomes only: no replay, raw reasons or export approval.
use crate::read_model::ReadModelError;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use skilltape_verify::{Receipt, RunContext};
use std::collections::{BTreeMap, BTreeSet};

const MAX_ROWS: usize = 1000;
const MAX_TEXT: usize = 4096;
const DETAIL_LIMIT: usize = 64;
type Key = (&'static str, String, String, Option<String>);
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
struct Outcome {
    status: String,
    exit_code: Option<i32>,
}
type Outcomes = BTreeMap<Outcome, usize>;
type Groups = BTreeMap<Key, Outcomes>;

pub(crate) struct SavedReceipt {
    pub id: String,
    pub document: Value,
    pub sha256: String,
    pub context_state: &'static str,
    pub context: Option<RunContext>,
}
#[derive(Debug, Serialize)]
pub struct ReceiptComparison {
    schema: &'static str,
    before: Identity,
    after: Identity,
    comparability: &'static str,
    condition_findings: Vec<&'static str>,
    counts: Counts,
    changes: Vec<Delta>,
    change_total: usize,
    changes_truncated: bool,
    execution: &'static str,
    provenance: &'static str,
    export_approval: &'static str,
}
#[derive(Debug, Serialize)]
struct Identity {
    id: String,
    receipt_sha256: String,
    run_id: String,
    skill_hash: String,
    status: String,
    context_state: &'static str,
}
#[derive(Debug, Default, Serialize)]
struct Counts {
    new_non_success: usize,
    no_longer_observed_non_success: usize,
    continuing_non_success: usize,
    total_groups: usize,
    outcome_groups_changed: usize,
}
#[derive(Debug, Serialize)]
struct OutcomeCount {
    status: String,
    exit_code: Option<i32>,
    count: usize,
}
#[derive(Debug, Serialize)]
struct Delta {
    key: String,
    category: &'static str,
    kind: String,
    target: String,
    phase: Option<String>,
    label_truncated: bool,
    before: Vec<OutcomeCount>,
    after: Vec<OutcomeCount>,
}

pub(crate) fn parse(bytes: &[u8]) -> Result<Value, ReadModelError> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| ReadModelError::InvalidDocument)?;
    if !Receipt::matches_schema(&value) {
        return Err(ReadModelError::InvalidDocument);
    }
    for field in ["steps", "assertions", "policy_decisions"] {
        let rows = value[field]
            .as_array()
            .ok_or(ReadModelError::InvalidDocument)?;
        if rows.len() > MAX_ROWS {
            return Err(ReadModelError::InvalidDocument);
        }
        for row in rows {
            if row
                .as_object()
                .ok_or(ReadModelError::InvalidDocument)?
                .values()
                .any(|v| v.as_str().is_some_and(|s| s.len() > MAX_TEXT))
            {
                return Err(ReadModelError::InvalidDocument);
            }
        }
    }
    let mut step_ids = BTreeSet::new();
    for row in value["steps"].as_array().unwrap() {
        if !step_ids.insert(row["step_id"].as_str().unwrap())
            || (!row["exit_code"].is_null()
                && row["exit_code"]
                    .as_i64()
                    .and_then(|n| i32::try_from(n).ok())
                    .is_none())
            || ["stdout_bytes", "stderr_bytes"]
                .iter()
                .any(|key| row[key].as_u64().is_none_or(|n| n > 9_007_199_254_740_991))
        {
            return Err(ReadModelError::InvalidDocument);
        }
    }
    if value["status"] == "succeeded"
        && (value["steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["status"] != "succeeded")
            || value["assertions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["passed"] != true)
            || value["policy_decisions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["allowed"] != true))
    {
        return Err(ReadModelError::InvalidDocument);
    }
    Ok(value)
}
fn groups(value: &Value) -> Groups {
    let mut result = Groups::new();
    for field in ["steps", "assertions", "policy_decisions"] {
        for row in value[field].as_array().unwrap() {
            let (key, status, exit_code) = match field {
                "steps" => (
                    (
                        "step",
                        "step".into(),
                        row["step_id"].as_str().unwrap().into(),
                        None,
                    ),
                    row["status"].as_str().unwrap(),
                    row["exit_code"].as_i64().map(|n| n as i32),
                ),
                "assertions" => (
                    (
                        "assertion",
                        row["kind"].as_str().unwrap().into(),
                        row["target"].as_str().unwrap().into(),
                        None,
                    ),
                    if row["passed"] == true {
                        "passed"
                    } else {
                        "failed"
                    },
                    None,
                ),
                _ => (
                    (
                        "policy",
                        row["code"].as_str().unwrap().into(),
                        row["step_id"].as_str().unwrap().into(),
                        Some(row["phase"].as_str().unwrap().into()),
                    ),
                    if row["allowed"] == true {
                        "allowed"
                    } else {
                        "denied"
                    },
                    None,
                ),
            };
            *result
                .entry(key)
                .or_default()
                .entry(Outcome {
                    status: status.into(),
                    exit_code,
                })
                .or_default() += 1;
        }
    }
    result
}
fn non_success(rows: &Outcomes) -> usize {
    rows.iter()
        .filter(|(outcome, _)| {
            !matches!(outcome.status.as_str(), "succeeded" | "passed" | "allowed")
        })
        .map(|(_, count)| count)
        .sum()
}
fn outcomes(rows: &Outcomes) -> Vec<OutcomeCount> {
    rows.iter()
        .map(|(outcome, count)| OutcomeCount {
            status: outcome.status.clone(),
            exit_code: outcome.exit_code,
            count: *count,
        })
        .collect()
}
fn label(value: &str) -> (String, bool) {
    let truncated = value.chars().count() > 240;
    (
        value.chars().take(240).collect::<String>() + if truncated { "…" } else { "" },
        truncated,
    )
}
fn identity(value: &SavedReceipt) -> Identity {
    Identity {
        id: value.id.clone(),
        receipt_sha256: value.sha256.clone(),
        run_id: value.document["run_id"].as_str().unwrap().into(),
        skill_hash: value.document["skill_hash"].as_str().unwrap().into(),
        status: value.document["status"].as_str().unwrap().into(),
        context_state: value.context_state,
    }
}

pub(crate) fn compare(
    before: SavedReceipt,
    after: SavedReceipt,
) -> Result<ReceiptComparison, ReadModelError> {
    let mut findings = Vec::new();
    if before.document["skill_hash"] != after.document["skill_hash"] {
        findings.push("package_changed");
    }
    if let (Some(a), Some(b)) = (&before.context, &after.context) {
        if a.input_set_sha256 != b.input_set_sha256 {
            findings.push("input_changed");
        }
        if a.executor != b.executor || a.executor_version != b.executor_version {
            findings.push("executor_changed");
        }
        if a.os != b.os || a.arch != b.arch {
            findings.push("platform_changed");
        }
        if (a.max_processes, a.step_timeout_ms, a.max_output_bytes)
            != (b.max_processes, b.step_timeout_ms, b.max_output_bytes)
        {
            findings.push("limits_changed");
        }
    }
    let different = !findings.is_empty();
    if before.context_state == "invalid" || after.context_state == "invalid" {
        findings.push("context_invalid");
    }
    if before.context_state == "unrecorded" || after.context_state == "unrecorded" {
        findings.push("conditions_unrecorded");
    }
    let comparability = if different {
        "incomparable"
    } else if before.context.is_some() && after.context.is_some() {
        "same-recorded-conditions"
    } else {
        "unknown"
    };
    let old = groups(&before.document);
    let new = groups(&after.document);
    let keys: BTreeSet<_> = old.keys().chain(new.keys()).cloned().collect();
    let mut counts = Counts {
        total_groups: keys.len(),
        ..Counts::default()
    };
    let empty = Outcomes::new();
    let mut changes = Vec::new();
    let mut change_total = 0;
    for key in keys {
        let a = old.get(&key).unwrap_or(&empty);
        let b = new.get(&key).unwrap_or(&empty);
        let (prior, current) = (non_success(a), non_success(b));
        counts.new_non_success += current.saturating_sub(prior);
        counts.no_longer_observed_non_success += prior.saturating_sub(current);
        counts.continuing_non_success += prior.min(current);
        if a != b {
            counts.outcome_groups_changed += 1;
        }
        if a == b && prior == 0 {
            continue;
        }
        change_total += 1;
        if changes.len() >= DETAIL_LIMIT {
            continue;
        }
        let digest =
            Sha256::digest(serde_json::to_vec(&key).map_err(|_| ReadModelError::InvalidDocument)?);
        let (kind, kind_truncated) = label(&key.1);
        let (target, target_truncated) = label(&key.2);
        changes.push(Delta {
            key: digest.iter().map(|b| format!("{b:02x}")).collect(),
            category: key.0,
            kind,
            target,
            phase: key.3,
            label_truncated: kind_truncated || target_truncated,
            before: outcomes(a),
            after: outcomes(b),
        });
    }
    Ok(ReceiptComparison {
        schema: "skilltape.dev/receipt-comparison/v1",
        before: identity(&before),
        after: identity(&after),
        comparability,
        condition_findings: findings,
        counts,
        changes_truncated: change_total > changes.len(),
        changes,
        change_total,
        execution: "not-run",
        provenance: "not-authenticated",
        export_approval: "not-granted",
    })
}
