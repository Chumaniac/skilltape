use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use skilltape_schema::{validate_json, SchemaId, Step};
use thiserror::Error;

use crate::{CompileOutput, StepProvenance};

/// The immutable capabilities a proposal is allowed to use.
///
/// This is deliberately derived from the deterministic base. A proposal cannot
/// change permissions and then use the changed permissions to authorize itself.
#[derive(Clone, Debug)]
pub struct ProposalPolicy {
    allowed_executables: BTreeSet<String>,
    read_scopes: Vec<String>,
    write_scopes: Vec<String>,
    event_sequences: BTreeSet<u64>,
    network_enabled: bool,
    secrets_enabled: bool,
}

impl ProposalPolicy {
    pub fn from_base(base: &CompileOutput) -> Self {
        Self {
            allowed_executables: base
                .permissions
                .process
                .executables
                .iter()
                .cloned()
                .collect(),
            read_scopes: base.permissions.filesystem.read.clone(),
            write_scopes: base.permissions.filesystem.write.clone(),
            event_sequences: base
                .provenance
                .iter()
                .flat_map(|source| source.event_sequences.iter().copied())
                .collect(),
            network_enabled: base.permissions.network.enabled,
            secrets_enabled: base.permissions.secrets.read_environment,
        }
    }
}

#[derive(Debug, Error)]
pub enum ProposalError {
    #[error("proposal is pending human confirmation")]
    Pending,
    #[error("proposal was rejected")]
    Rejected,
    #[error("proposal input hash is stale: expected {expected}, got {actual}")]
    StaleInputHash { expected: String, actual: String },
    #[error("proposal patch must be a JSON object")]
    MalformedPatch,
    #[error("proposal patch contains unsupported key `{key}`")]
    UnknownPatchKey { key: String },
    #[error("proposal attempts to change permissions")]
    PermissionWidening,
    #[error("proposal workflow is invalid: {reason}")]
    MalformedWorkflow { reason: String },
    #[error("proposal provenance is invalid: {reason}")]
    MalformedProvenance { reason: String },
    #[error("proposal is missing provenance for step `{step_id}`")]
    MissingProvenance { step_id: String },
    #[error("proposal uses undeclared executable `{program}`")]
    UndeclaredExecutable { program: String },
    #[error("proposal uses a network capability that is not allowed")]
    NetworkNotAllowed,
    #[error("proposal uses secret or environment access that is not allowed")]
    SecretsNotAllowed,
    #[error("proposal path is unsafe: `{path}`")]
    UnsafePath { path: String },
    #[error("proposal path is outside the deterministic base scopes: `{path}`")]
    PathOutsideBase { path: String },
    #[error(
        "proposal provenance references event sequence {event_sequence} outside the base policy"
    )]
    EventSequenceOutsidePolicy { event_sequence: u64 },
    #[error("proposal serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Applies a provider proposal without allowing it to expand the deterministic
/// base's execution authority.
pub fn apply_proposal(
    base: CompileOutput,
    proposal: crate::WorkflowProposal,
    policy: &ProposalPolicy,
) -> Result<CompileOutput, ProposalError> {
    match proposal.status {
        crate::ProposalStatus::Pending => return Err(ProposalError::Pending),
        crate::ProposalStatus::Rejected => return Err(ProposalError::Rejected),
        crate::ProposalStatus::Accepted => {}
    }

    let expected_hash = base
        .content_hash()
        .map_err(|_| ProposalError::MalformedPatch)?;
    if proposal.input_hash != expected_hash {
        return Err(ProposalError::StaleInputHash {
            expected: expected_hash,
            actual: proposal.input_hash,
        });
    }

    let patch = proposal
        .workflow_patch
        .as_object()
        .ok_or(ProposalError::MalformedPatch)?;
    for key in patch.keys() {
        if key != "workflow" && key != "provenance" {
            if key == "permissions" || key == "network" || key == "secrets" {
                return Err(ProposalError::PermissionWidening);
            }
            return Err(ProposalError::UnknownPatchKey { key: key.clone() });
        }
    }

    let workflow = match patch.get("workflow") {
        Some(value) => parse_workflow(value)?,
        None => base.workflow.clone(),
    };
    let provenance = match patch.get("provenance") {
        Some(value) => parse_provenance(value)?,
        None if patch.contains_key("workflow") => preserve_provenance(&base, &workflow)?,
        None => base.provenance.clone(),
    };

    let candidate = CompileOutput {
        workflow,
        permissions: base.permissions.clone(),
        skill_markdown: append_descriptions(&base.skill_markdown, &proposal.descriptions),
        fixtures: base.fixtures.clone(),
        provenance,
    };

    // CompileOutput's validated deserializer enforces workflow ids, one
    // provenance record per step, and event ordering before policy checks.
    let candidate = serde_json::from_value::<CompileOutput>(serde_json::to_value(candidate)?)
        .map_err(|error| ProposalError::MalformedProvenance {
            reason: error.to_string(),
        })?;

    validate_event_sequences(&candidate.provenance, policy)?;
    validate_workflow_capabilities(&candidate.workflow, policy)?;
    Ok(candidate)
}

fn parse_workflow(value: &Value) -> Result<skilltape_schema::Workflow, ProposalError> {
    reject_unknown_workflow_fields(value)?;
    if let Err(errors) = validate_json(SchemaId::WorkflowV1, value) {
        // Let the policy layer report unsafe paths with its typed error. All
        // other schema violations (unknown actions/fields, wrong types, etc.)
        // are rejected before a candidate can reach capability checks.
        if errors.iter().any(|error| {
            matches!(
                error.keyword.as_str(),
                "additionalProperties" | "const" | "enum" | "minLength" | "required" | "type"
            )
        }) {
            return Err(ProposalError::MalformedWorkflow {
                reason: errors
                    .into_iter()
                    .map(|error| error.message)
                    .collect::<Vec<_>>()
                    .join("; "),
            });
        }
    }
    serde_json::from_value(value.clone()).map_err(|error| ProposalError::MalformedWorkflow {
        reason: error.to_string(),
    })
}

fn reject_unknown_workflow_fields(value: &Value) -> Result<(), ProposalError> {
    let object = value
        .as_object()
        .ok_or_else(|| malformed_workflow("workflow must be an object"))?;
    reject_unknown_keys(object, &["schema", "steps"], "workflow")?;
    let steps = object
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| malformed_workflow("workflow.steps must be an array"))?;

    for (index, step) in steps.iter().enumerate() {
        let step_object = step.as_object().ok_or_else(|| {
            malformed_workflow(format!("workflow.steps[{index}] must be an object"))
        })?;
        let action = step_object
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                malformed_workflow(format!("workflow.steps[{index}].action is required"))
            })?;
        let allowed = match action {
            "exec" => ["action", "id", "program", "args", "timeout_ms", "outputs"].as_slice(),
            "script" => ["action", "id", "path", "args", "timeout_ms", "outputs"].as_slice(),
            "file" => ["action", "id", "operation", "from", "to"].as_slice(),
            "assert" => ["action", "id", "assertion"].as_slice(),
            other => {
                return Err(malformed_workflow(format!(
                    "workflow.steps[{index}] has unsupported action `{other}`"
                )))
            }
        };
        reject_unknown_keys(step_object, allowed, &format!("workflow.steps[{index}"))?;

        if let Some(outputs) = step_object.get("outputs") {
            let outputs = outputs.as_array().ok_or_else(|| {
                malformed_workflow(format!("workflow.steps[{index}].outputs must be an array"))
            })?;
            for (output_index, output) in outputs.iter().enumerate() {
                let output = output.as_object().ok_or_else(|| {
                    malformed_workflow(format!(
                        "workflow.steps[{index}].outputs[{output_index}] must be an object"
                    ))
                })?;
                reject_unknown_keys(
                    output,
                    &["path", "type"],
                    &format!("workflow.steps[{index}].outputs[{output_index}"),
                )?;
            }
        }

        if action == "assert" {
            let assertion = step_object
                .get("assertion")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    malformed_workflow(format!(
                        "workflow.steps[{index}].assertion must be an object"
                    ))
                })?;
            reject_unknown_keys(
                assertion,
                &["type", "path", "schema", "hash"],
                &format!("workflow.steps[{index}].assertion"),
            )?;
        }
    }
    Ok(())
}

fn reject_unknown_keys(
    object: &serde_json::Map<String, Value>,
    allowed: &[&str],
    context: &str,
) -> Result<(), ProposalError> {
    if let Some(key) = object
        .keys()
        .find(|key| !allowed.contains(&key.as_str()) && !key.starts_with("x-"))
    {
        return Err(malformed_workflow(format!(
            "{context} contains unsupported field `{key}`"
        )));
    }
    Ok(())
}

fn malformed_workflow(reason: impl Into<String>) -> ProposalError {
    ProposalError::MalformedWorkflow {
        reason: reason.into(),
    }
}

fn parse_provenance(value: &Value) -> Result<Vec<StepProvenance>, ProposalError> {
    serde_json::from_value(value.clone()).map_err(|error| ProposalError::MalformedProvenance {
        reason: error.to_string(),
    })
}

fn preserve_provenance(
    base: &CompileOutput,
    workflow: &skilltape_schema::Workflow,
) -> Result<Vec<StepProvenance>, ProposalError> {
    let by_step = base
        .provenance
        .iter()
        .cloned()
        .map(|source| (source.step_id.clone(), source))
        .collect::<BTreeMap<_, _>>();
    workflow
        .steps
        .iter()
        .map(|step| {
            let step_id = step_id(step).to_owned();
            by_step
                .get(&step_id)
                .cloned()
                .ok_or(ProposalError::MissingProvenance { step_id })
        })
        .collect()
}

fn append_descriptions(base: &str, descriptions: &BTreeMap<String, String>) -> String {
    if descriptions.is_empty() {
        return base.to_owned();
    }
    let mut result = base.trim_end_matches('\n').to_owned();
    result.push_str("\n\n## Proposal notes\n\n");
    for (step_id, description) in descriptions {
        result.push_str("- `");
        result.push_str(step_id);
        result.push_str("`: ");
        result.push_str(description);
        result.push('\n');
    }
    result
}

fn validate_event_sequences(
    provenance: &[StepProvenance],
    policy: &ProposalPolicy,
) -> Result<(), ProposalError> {
    for source in provenance {
        for sequence in &source.event_sequences {
            if !policy.event_sequences.contains(sequence) {
                return Err(ProposalError::EventSequenceOutsidePolicy {
                    event_sequence: *sequence,
                });
            }
        }
    }
    Ok(())
}

fn validate_workflow_capabilities(
    workflow: &skilltape_schema::Workflow,
    policy: &ProposalPolicy,
) -> Result<(), ProposalError> {
    for step in &workflow.steps {
        match step {
            Step::Exec(step) => {
                if !policy.allowed_executables.contains(&step.program) {
                    return Err(ProposalError::UndeclaredExecutable {
                        program: step.program.clone(),
                    });
                }
                validate_arguments(&step.args, policy)?;
                for output in &step.outputs {
                    validate_path(&output.path, &policy.write_scopes)?;
                }
            }
            Step::Script(step) => {
                validate_path(&step.path, &policy.read_scopes)?;
                validate_arguments(&step.args, policy)?;
                for output in &step.outputs {
                    validate_path(&output.path, &policy.write_scopes)?;
                }
            }
            Step::File(step) => {
                validate_path(&step.from_path, &policy.read_scopes)?;
                validate_path(&step.to_path, &policy.write_scopes)?;
            }
            Step::Assert(step) => {
                if let Some(path) = &step.assertion.path {
                    validate_path(path, &policy.read_scopes)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_arguments(args: &[String], policy: &ProposalPolicy) -> Result<(), ProposalError> {
    for arg in args {
        if contains_network_reference(arg) && !policy.network_enabled {
            return Err(ProposalError::NetworkNotAllowed);
        }
        if contains_secret_reference(arg) && !policy.secrets_enabled {
            return Err(ProposalError::SecretsNotAllowed);
        }
        if let Some(path) = unsafe_argument_path(arg) {
            return Err(ProposalError::UnsafePath { path });
        }
    }
    Ok(())
}

fn validate_path(path: &str, scopes: &[String]) -> Result<(), ProposalError> {
    if !is_safe_relative_path(path) {
        return Err(ProposalError::UnsafePath {
            path: path.to_owned(),
        });
    }
    if !scopes.iter().any(|scope| path_matches_scope(path, scope)) {
        return Err(ProposalError::PathOutsideBase {
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn path_matches_scope(path: &str, scope: &str) -> bool {
    if path == scope {
        return true;
    }
    scope
        .strip_suffix("/**")
        .map(|prefix| path.starts_with(prefix) && path.as_bytes().get(prefix.len()) == Some(&b'/'))
        .unwrap_or(false)
}

fn is_safe_relative_path(path: &str) -> bool {
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || (path.len() >= 2 && path.as_bytes()[1] == b':')
    {
        return false;
    }
    let normalized = path.replace('\\', "/");
    let components = normalized
        .split('/')
        .filter(|component| !component.is_empty());
    let mut has_component = false;
    for component in components {
        if component == ".."
            || component == "."
            || component.contains('\0')
            || component.chars().any(char::is_control)
        {
            return false;
        }
        has_component = true;
    }
    has_component
}

fn contains_network_reference(value: &str) -> bool {
    [
        "http://",
        "https://",
        "ftp://",
        "tcp://",
        "udp://",
        "socket",
        "urllib",
        "requests",
        "curl",
        "wget",
        "fetch(",
        "download(",
    ]
    .iter()
    .any(|prefix| value.to_ascii_lowercase().contains(prefix))
}

fn contains_secret_reference(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    if lowered.contains("{{ secrets.")
        || lowered.contains("{{ env.")
        || lowered.contains("os.environ")
        || lowered.contains("getenv(")
        || lowered.contains("secret")
        || lowered.contains("token")
    {
        return true;
    }
    let bytes = value.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'$' {
            let next = bytes.get(index + 1);
            if next == Some(&b'{') || next.is_some_and(|byte| byte.is_ascii_uppercase()) {
                return true;
            }
        }
    }
    false
}

fn unsafe_argument_path(value: &str) -> Option<String> {
    for token in
        value.split(|character: char| character.is_whitespace() || "\"'`()=,;".contains(character))
    {
        if token.is_empty() || token.starts_with("http:") || token.starts_with("https:") {
            continue;
        }
        if token.starts_with('/')
            || token.starts_with('\\')
            || (token.len() >= 2 && token.as_bytes()[1] == b':')
            || token.split(['/', '\\']).any(|component| component == "..")
        {
            return Some(token.to_owned());
        }
    }
    None
}

fn step_id(step: &Step) -> &str {
    match step {
        Step::Exec(step) => &step.id,
        Step::Script(step) => &step.id,
        Step::File(step) => &step.id,
        Step::Assert(step) => &step.id,
    }
}
