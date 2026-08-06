use std::collections::BTreeMap;

use skilltape_compiler::{
    CompileError, CompileOutput, CompileRequest, CompileTarget, FixtureDraft, StepProvenance,
    TapeSession,
};
use skilltape_schema::{
    ExecStep, FilesystemPermissions, NetworkPermissions, Permissions, ProcessPermissions,
    SecretPermissions, Step, Workflow,
};
use skilltape_tape::{EventSource, RedactionState, TapeEvent, TapeEventKind};

fn event(sequence: u64) -> TapeEvent {
    TapeEvent {
        sequence,
        occurred_at_ms: 1_000 + sequence,
        kind: TapeEventKind::TerminalCommand,
        source: EventSource::Cli,
        payload: serde_json::json!({"sequence": sequence}),
        redaction: RedactionState::Unredacted,
    }
}

fn session(count: u64) -> TapeSession {
    TapeSession::new((0..count).map(event).collect()).expect("valid tape session")
}

fn workflow() -> Workflow {
    Workflow {
        schema: "skilltape.dev/workflow/v1".into(),
        steps: vec![
            Step::Exec(ExecStep {
                id: "first".into(),
                program: "echo".into(),
                args: vec!["first".into()],
                timeout_ms: 1_000,
                outputs: Vec::new(),
            }),
            Step::Exec(ExecStep {
                id: "second".into(),
                program: "echo".into(),
                args: vec!["second".into()],
                timeout_ms: 1_000,
                outputs: Vec::new(),
            }),
        ],
    }
}

fn permissions() -> Permissions {
    Permissions {
        schema: "skilltape.dev/permissions/v1".into(),
        filesystem: FilesystemPermissions {
            read: vec!["workspace".into()],
            write: vec!["workspace".into()],
        },
        process: ProcessPermissions {
            executables: vec!["echo".into()],
            max_processes: 1,
            default_timeout_ms: 1_000,
        },
        network: NetworkPermissions {
            enabled: false,
            allow_hosts: Vec::new(),
        },
        secrets: SecretPermissions {
            read_environment: false,
        },
    }
}

fn fixture_draft() -> FixtureDraft {
    FixtureDraft::new(BTreeMap::from([("README.txt".into(), "fixture".into())]))
}

fn output(
    tape: &TapeSession,
    provenance: Vec<StepProvenance>,
) -> Result<CompileOutput, CompileError> {
    output_with(tape, workflow(), permissions(), provenance)
}

fn output_with(
    tape: &TapeSession,
    workflow: Workflow,
    permissions: Permissions,
    provenance: Vec<StepProvenance>,
) -> Result<CompileOutput, CompileError> {
    CompileOutput::try_new(
        tape,
        workflow,
        permissions,
        "# Skill".into(),
        fixture_draft(),
        provenance,
    )
}

fn output_value(provenance: Vec<StepProvenance>) -> serde_json::Value {
    serde_json::to_value(output(&session(2), provenance).expect("valid output")).unwrap()
}

#[test]
fn every_workflow_step_requires_nonempty_provenance() {
    let error = output(
        &session(2),
        vec![StepProvenance::new("first", vec![0], "first command").unwrap()],
    )
    .expect_err("second step must have a source");

    assert!(matches!(error, CompileError::MissingSource { step_id } if step_id == "second"));
}

#[test]
fn unknown_provenance_sequence_is_typed() {
    let error = output(
        &session(1),
        vec![
            StepProvenance::new("first", vec![0], "first command").unwrap(),
            StepProvenance::new("second", vec![9], "missing command").unwrap(),
        ],
    )
    .expect_err("sequence 9 is not in the tape");

    assert!(matches!(
        error,
        CompileError::UnknownSource {
            step_id,
            event_sequence: 9
        } if step_id == "second"
    ));
}

#[test]
fn duplicate_and_out_of_order_sources_are_typed() {
    let duplicate = StepProvenance::new("first", vec![0, 0], "duplicate").unwrap_err();
    assert!(matches!(
        duplicate,
        CompileError::DuplicateSource {
            step_id,
            event_sequence: 0
        } if step_id == "first"
    ));

    let out_of_order = StepProvenance::new("first", vec![1, 0], "out of order").unwrap_err();
    assert!(matches!(
        out_of_order,
        CompileError::OutOfOrderSource {
            step_id,
            previous: 1,
            next: 0
        } if step_id == "first"
    ));
}

#[test]
fn provenance_is_canonicalized_to_workflow_order() {
    let result = output(
        &session(2),
        vec![
            StepProvenance::new("second", vec![1], "second command").unwrap(),
            StepProvenance::new("first", vec![0], "first command").unwrap(),
        ],
    )
    .expect("valid provenance should be accepted");

    assert_eq!(
        result
            .provenance
            .iter()
            .map(|source| source.step_id.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
}

#[test]
fn deterministic_serialization_and_hash_are_stable() {
    let target = CompileTarget::new("shell", "v1").expect("valid target");
    assert_eq!(target.identity(), "shell@v1");

    let first = output(
        &session(2),
        vec![
            StepProvenance::new("second", vec![1], "second command").unwrap(),
            StepProvenance::new("first", vec![0], "first command").unwrap(),
        ],
    )
    .unwrap();
    let second = output(
        &session(2),
        vec![
            StepProvenance::new("first", vec![0], "first command").unwrap(),
            StepProvenance::new("second", vec![1], "second command").unwrap(),
        ],
    )
    .unwrap();

    assert_eq!(
        first.deterministic_json().unwrap(),
        second.deterministic_json().unwrap()
    );
    assert_eq!(
        first.content_hash().unwrap(),
        second.content_hash().unwrap()
    );
}

#[test]
fn compile_output_round_trips_as_json() {
    let value = output(
        &session(2),
        vec![
            StepProvenance::new("first", vec![0], "first command").unwrap(),
            StepProvenance::new("second", vec![1], "second command").unwrap(),
        ],
    )
    .unwrap();
    let bytes = value.deterministic_json().unwrap();
    let round_trip: CompileOutput = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(
        serde_json::to_vec(&value).unwrap(),
        serde_json::to_vec(&round_trip).unwrap()
    );
}

#[test]
fn deserialized_output_canonicalizes_provenance_to_workflow_order() {
    let mut value = output_value(vec![
        StepProvenance::new("first", vec![0], "first command").unwrap(),
        StepProvenance::new("second", vec![1], "second command").unwrap(),
    ]);
    let provenance = value["provenance"].as_array().unwrap().clone();
    value["provenance"] = serde_json::json!([provenance[1], provenance[0]]);

    let loaded: CompileOutput = serde_json::from_value(value).expect("valid output");

    assert_eq!(
        loaded
            .provenance
            .iter()
            .map(|source| source.step_id.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
}

#[test]
fn deserialized_output_rejects_missing_provenance() {
    let mut value = output_value(vec![
        StepProvenance::new("first", vec![0], "first command").unwrap(),
        StepProvenance::new("second", vec![1], "second command").unwrap(),
    ]);
    value["provenance"] = serde_json::json!([value["provenance"][0].clone()]);

    assert!(serde_json::from_value::<CompileOutput>(value).is_err());
}

#[test]
fn deserialized_output_rejects_duplicate_provenance() {
    let mut value = output_value(vec![
        StepProvenance::new("first", vec![0], "first command").unwrap(),
        StepProvenance::new("second", vec![1], "second command").unwrap(),
    ]);
    let first = value["provenance"][0].clone();
    value["provenance"] = serde_json::json!([
        first.clone(),
        first,
        value["provenance"][1].clone()
    ]);

    assert!(serde_json::from_value::<CompileOutput>(value).is_err());
}

#[test]
fn deserialized_output_rejects_unknown_workflow_step_provenance() {
    let mut value = output_value(vec![
        StepProvenance::new("first", vec![0], "first command").unwrap(),
        StepProvenance::new("second", vec![1], "second command").unwrap(),
    ]);
    let mut provenance = value["provenance"].as_array().unwrap().clone();
    provenance.push(serde_json::json!({
        "step_id": "unknown",
        "event_sequences": [0],
        "source_summary": "unknown command"
    }));
    value["provenance"] = serde_json::Value::Array(provenance);

    assert!(serde_json::from_value::<CompileOutput>(value).is_err());
}

#[test]
fn deserialized_output_rejects_out_of_order_event_provenance() {
    let mut value = output_value(vec![
        StepProvenance::new("first", vec![0], "first command").unwrap(),
        StepProvenance::new("second", vec![1], "second command").unwrap(),
    ]);
    value["provenance"][0]["event_sequences"] = serde_json::json!([1, 0]);

    assert!(serde_json::from_value::<CompileOutput>(value).is_err());
}

#[test]
fn deserialized_output_rejects_empty_event_provenance() {
    let mut value = output_value(vec![
        StepProvenance::new("first", vec![0], "first command").unwrap(),
        StepProvenance::new("second", vec![1], "second command").unwrap(),
    ]);
    value["provenance"][0]["event_sequences"] = serde_json::json!([]);

    assert!(serde_json::from_value::<CompileOutput>(value).is_err());
}

#[test]
fn deserialized_output_defers_tape_reference_validation_to_validate() {
    let mut value = output_value(vec![
        StepProvenance::new("first", vec![0], "first command").unwrap(),
        StepProvenance::new("second", vec![1], "second command").unwrap(),
    ]);
    value["provenance"][1]["event_sequences"] = serde_json::json!([99]);

    let loaded: CompileOutput = serde_json::from_value(value).expect("structurally valid output");
    let error = loaded
        .validate(&session(2))
        .expect_err("tape reference must be validated with a session");

    assert!(matches!(
        error,
        CompileError::UnknownSource {
            step_id,
            event_sequence: 99
        } if step_id == "second"
    ));
}

#[test]
fn compile_target_deserialization_enforces_constructor_invariants() {
    for value in [
        serde_json::json!({"name": "", "version": "v1"}),
        serde_json::json!({"name": "shell", "version": ""}),
        serde_json::json!({"name": "shell@unsafe", "version": "v1"}),
        serde_json::json!({"name": "shell", "version": "v@1"}),
    ] {
        assert!(serde_json::from_value::<CompileTarget>(value).is_err());
    }
}

#[test]
fn compile_request_deserialization_enforces_constructor_invariants() {
    let request = CompileRequest::new(
        session(1),
        "compile",
        CompileTarget::new("shell", "v1").unwrap(),
    )
    .unwrap();
    let mut value = serde_json::to_value(request).unwrap();
    value["name"] = serde_json::json!("");

    assert!(serde_json::from_value::<CompileRequest>(value).is_err());

    let request = CompileRequest::new(
        session(1),
        "compile",
        CompileTarget::new("shell", "v1").unwrap(),
    )
    .unwrap();
    let mut value = serde_json::to_value(request).unwrap();
    value["target"]["name"] = serde_json::json!("shell@unsafe");

    assert!(serde_json::from_value::<CompileRequest>(value).is_err());
}

#[test]
fn permission_collection_order_and_duplicates_do_not_change_hash() {
    let mut first_permissions = permissions();
    first_permissions.filesystem.read = vec!["zeta".into(), "alpha".into(), "zeta".into()];
    first_permissions.filesystem.write = vec!["write-b".into(), "write-a".into()];
    first_permissions.process.executables = vec!["zsh".into(), "bash".into(), "zsh".into()];
    first_permissions.network.allow_hosts = vec!["z.example".into(), "a.example".into()];

    let mut second_permissions = permissions();
    second_permissions.filesystem.read = vec!["alpha".into(), "zeta".into()];
    second_permissions.filesystem.write = vec!["write-a".into(), "write-b".into()];
    second_permissions.process.executables = vec!["bash".into(), "zsh".into()];
    second_permissions.network.allow_hosts = vec!["a.example".into(), "z.example".into()];

    let first = output_with(
        &session(2),
        workflow(),
        first_permissions,
        vec![
            StepProvenance::new("first", vec![0], "first command").unwrap(),
            StepProvenance::new("second", vec![1], "second command").unwrap(),
        ],
    )
    .unwrap();
    let second = output_with(
        &session(2),
        workflow(),
        second_permissions,
        vec![
            StepProvenance::new("first", vec![0], "first command").unwrap(),
            StepProvenance::new("second", vec![1], "second command").unwrap(),
        ],
    )
    .unwrap();

    assert_eq!(first.deterministic_json().unwrap(), second.deterministic_json().unwrap());
    assert_eq!(first.content_hash().unwrap(), second.content_hash().unwrap());
}

#[test]
fn permission_collection_canonicalization_does_not_mutate_output() {
    let mut unsorted_permissions = permissions();
    unsorted_permissions.filesystem.read = vec!["zeta".into(), "alpha".into()];
    unsorted_permissions.process.executables = vec!["zsh".into(), "bash".into()];

    let output = output_with(
        &session(2),
        workflow(),
        unsorted_permissions,
        vec![
            StepProvenance::new("first", vec![0], "first command").unwrap(),
            StepProvenance::new("second", vec![1], "second command").unwrap(),
        ],
    )
    .unwrap();
    let serialized_before_hash = serde_json::to_vec(&output).unwrap();

    assert_ne!(serialized_before_hash, output.deterministic_json().unwrap());
    assert_eq!(serialized_before_hash, serde_json::to_vec(&output).unwrap());
}

#[test]
fn deterministic_hash_preserves_workflow_and_command_order() {
    let ordered = output_with(
        &session(2),
        workflow(),
        permissions(),
        vec![
            StepProvenance::new("first", vec![0], "first command").unwrap(),
            StepProvenance::new("second", vec![1], "second command").unwrap(),
        ],
    )
    .unwrap();

    let mut reordered_workflow = workflow();
    reordered_workflow.steps.reverse();
    let reordered = output_with(
        &session(2),
        reordered_workflow,
        permissions(),
        vec![
            StepProvenance::new("second", vec![1], "second command").unwrap(),
            StepProvenance::new("first", vec![0], "first command").unwrap(),
        ],
    )
    .unwrap();

    assert_ne!(ordered.content_hash().unwrap(), reordered.content_hash().unwrap());

    let mut changed_args_workflow = workflow();
    if let Step::Exec(step) = &mut changed_args_workflow.steps[0] {
        step.args = vec!["value".into(), "first".into()];
    }
    let changed_args = output_with(
        &session(2),
        changed_args_workflow,
        permissions(),
        vec![
            StepProvenance::new("first", vec![0], "first command").unwrap(),
            StepProvenance::new("second", vec![1], "second command").unwrap(),
        ],
    )
    .unwrap();

    assert_ne!(ordered.content_hash().unwrap(), changed_args.content_hash().unwrap());
}

#[test]
fn content_hash_is_artifact_only_and_excludes_request_envelope() {
    let artifact = output(
        &session(2),
        vec![
            StepProvenance::new("first", vec![0], "first command").unwrap(),
            StepProvenance::new("second", vec![1], "second command").unwrap(),
        ],
    )
    .unwrap();
    let artifact_hash = artifact.content_hash().unwrap();

    let first_request = CompileRequest::new(
        session(2),
        "first-name",
        CompileTarget::new("shell", "v1").unwrap(),
    )
    .unwrap();
    let second_request = CompileRequest::new(
        session(2),
        "second-name",
        CompileTarget::new("python", "v2").unwrap(),
    )
    .unwrap();

    assert_ne!(first_request.name, second_request.name);
    assert_ne!(first_request.target.identity(), second_request.target.identity());
    assert_eq!(artifact_hash, artifact.content_hash().unwrap());
}

#[test]
fn provenance_serializes_in_compile_json_shape() {
    let target = CompileTarget::new("shell", "v1").unwrap();
    let provenance = vec![StepProvenance::new("first", vec![0], "first command").unwrap()];
    let document = skilltape_compiler::CompileProvenance::new(target, provenance);
    let value = serde_json::to_value(document).unwrap();

    assert_eq!(value["schema"], "skilltape.dev/compile/v1");
    assert_eq!(value["target_identity"], "shell@v1");
    assert_eq!(value["steps"][0]["event_sequences"][0], 0);
}
