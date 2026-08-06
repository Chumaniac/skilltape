use std::collections::BTreeMap;

use skilltape_compiler::{
    CompileError, CompileOutput, CompileTarget, FixtureDraft, StepProvenance, TapeSession,
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
    CompileOutput::try_new(
        tape,
        workflow(),
        permissions(),
        "# Skill".into(),
        fixture_draft(),
        provenance,
    )
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
fn provenance_serializes_in_compile_json_shape() {
    let target = CompileTarget::new("shell", "v1").unwrap();
    let provenance = vec![StepProvenance::new("first", vec![0], "first command").unwrap()];
    let document = skilltape_compiler::CompileProvenance::new(target, provenance);
    let value = serde_json::to_value(document).unwrap();

    assert_eq!(value["schema"], "skilltape.dev/compile/v1");
    assert_eq!(value["target_identity"], "shell@v1");
    assert_eq!(value["steps"][0]["event_sequences"][0], 0);
}
