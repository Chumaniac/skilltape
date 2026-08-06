use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use skilltape_schema::{AssertStep, AssertionSpec, ExecStep, FileStep, Step};
use skilltape_tape::{TapeEvent, TapeEventKind};

use crate::{CompileError, FixtureDraft, StepProvenance, TapeSession};

const TERMINAL_EVENT_KIND: &str = "terminal_command";
const FILESYSTEM_EVENT_KIND: &str = "filesystem_changed";

#[derive(Debug)]
pub(crate) struct StepCompilation {
    pub steps: Vec<Step>,
    pub provenance: Vec<StepProvenance>,
    pub read_scopes: BTreeSet<String>,
    pub write_scopes: BTreeSet<String>,
    pub executables: BTreeSet<String>,
    pub fixtures: FixtureDraft,
}

#[derive(Debug)]
enum Operation {
    Terminal(TerminalOperation),
    Filesystem(FilesystemOperation),
}

#[derive(Debug)]
struct TerminalOperation {
    program: String,
    args: Vec<String>,
    cwd_scope: String,
    reads_inputs: bool,
    event_sequences: Vec<u64>,
}

#[derive(Debug)]
struct FilesystemOperation {
    path: String,
    kind: FilesystemOperationKind,
    previous_path: Option<String>,
    content_hash: Option<String>,
    size: Option<u64>,
    event_sequences: Vec<u64>,
}

#[derive(Clone, Copy, Debug)]
enum FilesystemOperationKind {
    Created,
    Modified,
    Moved,
    Deleted,
}

#[derive(Debug)]
struct PendingTerminal {
    start_sequence: u64,
    program: String,
    args: Vec<String>,
    cwd_scope: String,
    reads_inputs: bool,
}

pub(crate) fn compile_steps(tape: &TapeSession) -> Result<StepCompilation, CompileError> {
    let mut operations = Vec::new();
    let mut filesystem_operations = BTreeMap::new();
    let mut pending_terminal = None;

    for event in tape.events() {
        match &event.kind {
            TapeEventKind::TerminalCommand => {
                handle_terminal_event(event, &mut pending_terminal, &mut operations)?;
            }
            TapeEventKind::FilesystemChanged => {
                if let Some(pending) = &pending_terminal {
                    return Err(CompileError::AmbiguousTerminalGrouping {
                        sequence: pending.start_sequence,
                        reason: format!(
                            "filesystem event {} separates terminal start from output",
                            event.sequence
                        ),
                    });
                }
                let change = parse_filesystem_event(event)?;
                let operation_index = match filesystem_operations.get(&change.path).copied() {
                    Some(index)
                        if !matches!(change.kind, FilesystemOperationKind::Moved)
                            && !matches!(
                                operations.get(index),
                                Some(Operation::Filesystem(operation))
                                    if matches!(
                                        operation.kind,
                                        FilesystemOperationKind::Moved
                                    )
                            ) =>
                    {
                        index
                    }
                    _ => {
                        let index = operations.len();
                        let path = change.path.clone();
                        operations.push(Operation::Filesystem(change));
                        filesystem_operations.insert(path, index);
                        index
                    }
                };

                if let Some(Operation::Filesystem(operation)) = operations.get_mut(operation_index)
                {
                    if operation.event_sequences.last() != Some(&event.sequence) {
                        let change = parse_filesystem_event(event)?;
                        operation.kind = change.kind;
                        operation.previous_path = change.previous_path;
                        operation.content_hash = change.content_hash;
                        operation.size = change.size;
                        operation.event_sequences.push(event.sequence);
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(pending) = pending_terminal {
        return Err(CompileError::AmbiguousTerminalGrouping {
            sequence: pending.start_sequence,
            reason: "terminal started event has no adjacent output event".into(),
        });
    }

    build_compilation(operations)
}

fn handle_terminal_event(
    event: &TapeEvent,
    pending_terminal: &mut Option<PendingTerminal>,
    operations: &mut Vec<Operation>,
) -> Result<(), CompileError> {
    let object = payload_object(event, TERMINAL_EVENT_KIND)?;
    let phase = required_string(object, event, TERMINAL_EVENT_KIND, "phase")?;

    match phase {
        "started" => {
            if let Some(pending) = pending_terminal {
                return Err(CompileError::AmbiguousTerminalGrouping {
                    sequence: event.sequence,
                    reason: format!(
                        "terminal started event follows unclosed terminal event {}",
                        pending.start_sequence
                    ),
                });
            }
            let command = required_string(object, event, TERMINAL_EVENT_KIND, "command")?;
            if command.trim().is_empty() {
                return Err(malformed(event, TERMINAL_EVENT_KIND, "command"));
            }
            let args = parse_arguments(object, event)?;
            let cwd = required_string(object, event, TERMINAL_EVENT_KIND, "cwd")?;
            let cwd_scope = cwd_scope(event.sequence, cwd)?;
            let reads_inputs = args
                .iter()
                .any(|argument| contains_input_reference(argument));
            *pending_terminal = Some(PendingTerminal {
                start_sequence: event.sequence,
                program: command.to_owned(),
                args,
                cwd_scope,
                reads_inputs,
            });
        }
        "output" => {
            let pending =
                pending_terminal
                    .take()
                    .ok_or_else(|| CompileError::AmbiguousTerminalGrouping {
                        sequence: event.sequence,
                        reason: "terminal output event has no preceding started event".into(),
                    })?;
            validate_terminal_output(object, event)?;
            operations.push(Operation::Terminal(TerminalOperation {
                program: pending.program,
                args: pending.args,
                cwd_scope: pending.cwd_scope,
                reads_inputs: pending.reads_inputs,
                event_sequences: vec![pending.start_sequence, event.sequence],
            }));
        }
        other => {
            return Err(CompileError::UnsupportedPayload {
                sequence: event.sequence,
                event_kind: TERMINAL_EVENT_KIND.into(),
                value: format!("unsupported phase `{other}`"),
            });
        }
    }

    Ok(())
}

fn validate_terminal_output(
    object: &serde_json::Map<String, Value>,
    event: &TapeEvent,
) -> Result<(), CompileError> {
    let text = object
        .get("text")
        .ok_or_else(|| malformed(event, TERMINAL_EVENT_KIND, "text"))?;
    if !text.is_string() {
        return Err(malformed(event, TERMINAL_EVENT_KIND, "text"));
    }
    if let Some(truncated) = object.get("truncated") {
        if !truncated.is_boolean() {
            return Err(malformed(event, TERMINAL_EVENT_KIND, "truncated"));
        }
    }
    if let Some(redactions) = object.get("redactions") {
        if !redactions.is_array() {
            return Err(malformed(event, TERMINAL_EVENT_KIND, "redactions"));
        }
    }
    Ok(())
}

fn parse_arguments(
    object: &serde_json::Map<String, Value>,
    event: &TapeEvent,
) -> Result<Vec<String>, CompileError> {
    let value = object
        .get("args")
        .ok_or_else(|| malformed(event, TERMINAL_EVENT_KIND, "args"))?;
    let values = value
        .as_array()
        .ok_or_else(|| malformed(event, TERMINAL_EVENT_KIND, "args"))?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| malformed(event, TERMINAL_EVENT_KIND, &format!("args[{index}]")))
        })
        .collect()
}

fn parse_filesystem_event(event: &TapeEvent) -> Result<FilesystemOperation, CompileError> {
    let object = payload_object(event, FILESYSTEM_EVENT_KIND)?;
    let kind = required_string(object, event, FILESYSTEM_EVENT_KIND, "kind")?;
    let path = required_string(object, event, FILESYSTEM_EVENT_KIND, "path")?;
    let path = safe_relative_path(event.sequence, path)?;
    let previous_path = match object.get("previous_path") {
        None | Some(Value::Null) => None,
        Some(value) => {
            let previous_path = value
                .as_str()
                .ok_or_else(|| malformed(event, FILESYSTEM_EVENT_KIND, "previous_path"))?;
            Some(safe_relative_path(event.sequence, previous_path)?)
        }
    };
    let content_hash = optional_string(object, event, "content_hash")?;
    let size = match object.get("size") {
        None | Some(Value::Null) => None,
        Some(value) => value
            .as_u64()
            .ok_or_else(|| malformed(event, FILESYSTEM_EVENT_KIND, "size"))
            .map(Some)?,
    };

    let kind = match kind {
        "created" => FilesystemOperationKind::Created,
        "modified" => FilesystemOperationKind::Modified,
        "moved" => {
            if previous_path.is_none() {
                return Err(malformed(event, FILESYSTEM_EVENT_KIND, "previous_path"));
            }
            FilesystemOperationKind::Moved
        }
        "deleted" => FilesystemOperationKind::Deleted,
        other => {
            return Err(CompileError::UnsupportedPayload {
                sequence: event.sequence,
                event_kind: FILESYSTEM_EVENT_KIND.into(),
                value: format!("unsupported filesystem change kind `{other}`"),
            });
        }
    };

    Ok(FilesystemOperation {
        path,
        kind,
        previous_path,
        content_hash,
        size,
        event_sequences: vec![event.sequence],
    })
}

fn build_compilation(operations: Vec<Operation>) -> Result<StepCompilation, CompileError> {
    let mut steps = Vec::with_capacity(operations.len());
    let mut provenance = Vec::with_capacity(operations.len());
    let mut read_scopes = BTreeSet::new();
    let mut write_scopes = BTreeSet::new();
    let mut executables = BTreeSet::new();
    let mut fixture_files = BTreeMap::new();

    for (index, operation) in operations.into_iter().enumerate() {
        let step_number = index + 1;
        match operation {
            Operation::Terminal(operation) => {
                read_scopes.insert(operation.cwd_scope);
                if operation.reads_inputs {
                    read_scopes.insert("inputs/**".into());
                }
                executables.insert(operation.program.clone());
                let id = format!("exec-{step_number:04}");
                steps.push(Step::Exec(ExecStep {
                    id: id.clone(),
                    program: operation.program.clone(),
                    args: operation.args,
                    timeout_ms: 120_000,
                    outputs: Vec::new(),
                }));
                provenance.push(StepProvenance::new(
                    id,
                    operation.event_sequences,
                    format!("terminal command `{}`", operation.program),
                )?);
            }
            Operation::Filesystem(operation) => {
                let fixture_path = format!("fixtures/changes/{step_number:04}.json");
                fixture_files.insert(fixture_path, filesystem_fixture(&operation)?);
                let (id, step) = match operation.kind {
                    FilesystemOperationKind::Moved => {
                        let previous_path = operation.previous_path.clone().ok_or_else(|| {
                            CompileError::MalformedPayload {
                                sequence: operation.event_sequences[0],
                                event_kind: FILESYSTEM_EVENT_KIND.into(),
                                field: "previous_path".into(),
                            }
                        })?;
                        read_scopes.insert(previous_path.clone());
                        write_scopes.insert(operation.path.clone());
                        let id = format!("file-{step_number:04}");
                        let step = Step::File(FileStep {
                            id: id.clone(),
                            operation: "move".into(),
                            from_path: previous_path,
                            to_path: operation.path.clone(),
                        });
                        (id, step)
                    }
                    FilesystemOperationKind::Created | FilesystemOperationKind::Modified => {
                        read_scopes.insert(operation.path.clone());
                        let id = format!("assert-{step_number:04}");
                        let step = Step::Assert(AssertStep {
                            id: id.clone(),
                            assertion: AssertionSpec {
                                assertion_type: if operation.content_hash.is_some() {
                                    "file_hash".into()
                                } else {
                                    "file_exists".into()
                                },
                                path: Some(operation.path.clone()),
                                schema: None,
                                hash: operation.content_hash.clone(),
                            },
                        });
                        (id, step)
                    }
                    FilesystemOperationKind::Deleted => {
                        read_scopes.insert(operation.path.clone());
                        let id = format!("assert-{step_number:04}");
                        let step = Step::Assert(AssertStep {
                            id: id.clone(),
                            assertion: AssertionSpec {
                                assertion_type: "file_absent".into(),
                                path: Some(operation.path.clone()),
                                schema: None,
                                hash: None,
                            },
                        });
                        (id, step)
                    }
                };
                let source_summary = match operation.kind {
                    FilesystemOperationKind::Moved => format!(
                        "filesystem moved `{}` to `{}`",
                        operation.previous_path.as_deref().unwrap_or_default(),
                        operation.path
                    ),
                    FilesystemOperationKind::Created => {
                        format!("filesystem created `{}`", operation.path)
                    }
                    FilesystemOperationKind::Modified => {
                        format!("filesystem modified `{}`", operation.path)
                    }
                    FilesystemOperationKind::Deleted => {
                        format!("filesystem deleted `{}`", operation.path)
                    }
                };
                steps.push(step);
                provenance.push(StepProvenance::new(
                    id,
                    operation.event_sequences,
                    source_summary,
                )?);
            }
        }
    }

    Ok(StepCompilation {
        steps,
        provenance,
        read_scopes,
        write_scopes,
        executables,
        fixtures: FixtureDraft::new(fixture_files),
    })
}

fn filesystem_fixture(operation: &FilesystemOperation) -> Result<String, CompileError> {
    let value = serde_json::json!({
        "kind": filesystem_kind_name(operation.kind),
        "path": operation.path,
        "previous_path": operation.previous_path,
        "content_hash": operation.content_hash,
        "size": operation.size,
        "event_sequences": operation.event_sequences,
        "metadata_only": true,
    });
    Ok(serde_json::to_string_pretty(&value)?)
}

fn filesystem_kind_name(kind: FilesystemOperationKind) -> &'static str {
    match kind {
        FilesystemOperationKind::Created => "created",
        FilesystemOperationKind::Modified => "modified",
        FilesystemOperationKind::Moved => "moved",
        FilesystemOperationKind::Deleted => "deleted",
    }
}

fn payload_object<'a>(
    event: &'a TapeEvent,
    event_kind: &str,
) -> Result<&'a serde_json::Map<String, Value>, CompileError> {
    event
        .payload
        .as_object()
        .ok_or_else(|| malformed(event, event_kind, "payload"))
}

fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    event: &TapeEvent,
    event_kind: &str,
    field: &str,
) -> Result<&'a str, CompileError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| malformed(event, event_kind, field))
}

fn optional_string(
    object: &serde_json::Map<String, Value>,
    event: &TapeEvent,
    field: &str,
) -> Result<Option<String>, CompileError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .filter(|value| !value.is_empty())
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| malformed(event, FILESYSTEM_EVENT_KIND, field)),
    }
}

fn cwd_scope(sequence: u64, cwd: &str) -> Result<String, CompileError> {
    if is_absolute_path(cwd) {
        return Ok("workspace/**".into());
    }
    let path = safe_relative_path(sequence, cwd)?;
    Ok(format!("{path}/**"))
}

fn safe_relative_path(sequence: u64, path: &str) -> Result<String, CompileError> {
    if path.is_empty() || is_absolute_path(path) {
        return Err(CompileError::UnsafePath {
            sequence,
            path: path.into(),
        });
    }

    let normalized = path.replace('\\', "/");
    let mut components = Vec::new();
    for component in normalized.split('/') {
        if component == ".." || component.contains('\0') || component.chars().any(char::is_control)
        {
            return Err(CompileError::UnsafePath {
                sequence,
                path: path.into(),
            });
        }
        if component.is_empty() || component == "." {
            continue;
        }
        components.push(component);
    }

    if components.is_empty() {
        return Err(CompileError::UnsafePath {
            sequence,
            path: path.into(),
        });
    }
    Ok(components.join("/"))
}

fn is_absolute_path(path: &str) -> bool {
    path.starts_with('/')
        || path.starts_with('\\')
        || (path.len() >= 2 && path.as_bytes()[1] == b':')
}

fn contains_input_reference(value: &str) -> bool {
    let mut rest = value;
    while let Some(start) = rest.find("{{") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("}}") else {
            return false;
        };
        if rest[..end].trim().starts_with("inputs.") {
            return true;
        }
        rest = &rest[end + 2..];
    }
    false
}

fn malformed(event: &TapeEvent, event_kind: &str, field: &str) -> CompileError {
    CompileError::MalformedPayload {
        sequence: event.sequence,
        event_kind: event_kind.into(),
        field: field.into(),
    }
}
