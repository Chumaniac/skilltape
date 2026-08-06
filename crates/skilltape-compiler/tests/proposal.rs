use std::collections::BTreeMap;
use std::future::Future;
use std::pin::pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use serde_json::{json, Value};
use skilltape_compiler::{
    apply_proposal, CompileOutput, CompileRequest, CompileTarget, Compiler, DeterministicCompiler,
    ProposalError, ProposalInput, ProposalPolicy, ProposalProvider, ProposalStatus, ProviderError,
    TapeSession, WorkflowProposal,
};
use skilltape_schema::Step;
use skilltape_tape::{EventSource, RedactionState, TapeEvent, TapeEventKind};

fn event(sequence: u64, payload: Value) -> TapeEvent {
    TapeEvent {
        sequence,
        occurred_at_ms: 1_000 + sequence,
        kind: TapeEventKind::TerminalCommand,
        source: EventSource::Shell,
        payload,
        redaction: RedactionState::Redacted,
    }
}

fn base_output() -> CompileOutput {
    let tape = TapeSession::new(vec![
        event(
            0,
            json!({
                "phase": "started",
                "command": "python3",
                "args": ["-c", "print('done')"],
                "cwd": "work"
            }),
        ),
        event(
            1,
            json!({
                "phase": "output",
                "text": "done\n",
                "truncated": false,
                "redactions": []
            }),
        ),
    ])
    .expect("valid proposal test tape");
    let request = CompileRequest::new(
        tape,
        "proposal-skill",
        CompileTarget::new("generic-agent-skill", "0.1.0").expect("valid target"),
    )
    .expect("valid compile request");

    DeterministicCompiler
        .compile(request)
        .expect("deterministic base should compile")
}

fn accepted_proposal(base: &CompileOutput, workflow_patch: Value) -> WorkflowProposal {
    let mut proposal = WorkflowProposal::pending(
        workflow_patch,
        BTreeMap::new(),
        "fake-model",
        base.content_hash().expect("base hash"),
    );
    proposal.status = ProposalStatus::Accepted;
    proposal
}

fn workflow_patch(base: &CompileOutput, steps: Value, provenance: Option<Value>) -> Value {
    let mut patch = serde_json::Map::new();
    patch.insert(
        "workflow".into(),
        json!({
            "schema": base.workflow.schema,
            "steps": steps
        }),
    );
    if let Some(provenance) = provenance {
        patch.insert("provenance".into(), provenance);
    }
    Value::Object(patch)
}

fn replacement_step(args: &[&str]) -> Value {
    json!({
        "action": "exec",
        "id": "exec-0001",
        "program": "python3",
        "args": args,
        "timeout_ms": 120000,
        "outputs": []
    })
}

fn source(step_id: &str, event_sequences: &[u64]) -> Value {
    json!({
        "step_id": step_id,
        "event_sequences": event_sequences,
        "source_summary": "proposal test source"
    })
}

#[test]
fn proposal_input_hash_is_derived_from_the_base_and_provider_is_opt_in() {
    struct FakeProvider {
        calls: Arc<AtomicUsize>,
    }

    impl ProposalProvider for FakeProvider {
        async fn propose(&self, input: ProposalInput) -> Result<WorkflowProposal, ProviderError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(WorkflowProposal::pending(
                json!({}),
                BTreeMap::new(),
                "fake-model",
                input.input_hash,
            ))
        }
    }

    let base = base_output();
    let input = ProposalInput::from_base(&base).expect("proposal input should hash the base");
    assert_eq!(input.input_hash, base.content_hash().expect("base hash"));

    let calls = Arc::new(AtomicUsize::new(0));
    let provider = FakeProvider {
        calls: Arc::clone(&calls),
    };
    let _ = base
        .content_hash()
        .expect("deterministic compile remains provider-free");
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let proposal = block_on(provider.propose(input)).expect("fake provider succeeds");
    assert_eq!(proposal.input_hash, base.content_hash().expect("base hash"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn pending_constructor_and_status_round_trip_through_serde() {
    let proposal = WorkflowProposal::pending(json!({}), BTreeMap::new(), "fake-model", "base-hash");

    assert_eq!(proposal.status, ProposalStatus::Pending);
    let value = serde_json::to_value(&proposal).expect("proposal should serialize");
    assert_eq!(value["status"], "pending");
    let round_trip: WorkflowProposal =
        serde_json::from_value(value).expect("proposal should deserialize");
    assert_eq!(round_trip, proposal);
}

#[test]
fn provider_errors_are_typed_and_explanatory() {
    assert_eq!(ProviderError::Offline.to_string(), "provider is offline");
    assert_eq!(
        ProviderError::Timeout.to_string(),
        "provider request timed out"
    );
    assert_eq!(
        ProviderError::InvalidJson.to_string(),
        "provider returned invalid JSON"
    );
    assert_eq!(
        ProviderError::Quota.to_string(),
        "provider quota was exceeded"
    );
    assert_eq!(
        ProviderError::Failed {
            message: "service unavailable".into()
        }
        .to_string(),
        "provider failed: service unavailable"
    );
}

#[test]
fn pending_rejected_and_stale_proposals_fail_before_application() {
    let base = base_output();
    let policy = ProposalPolicy::from_base(&base);
    let hash = base.content_hash().expect("base hash");

    let pending = WorkflowProposal::pending(json!({}), BTreeMap::new(), "model", &hash);
    assert!(matches!(
        apply_proposal(base.clone(), pending, &policy),
        Err(ProposalError::Pending)
    ));

    let mut rejected = WorkflowProposal::pending(json!({}), BTreeMap::new(), "model", &hash);
    rejected.status = ProposalStatus::Rejected;
    assert!(matches!(
        apply_proposal(base.clone(), rejected, &policy),
        Err(ProposalError::Rejected)
    ));

    let mut stale = WorkflowProposal::pending(json!({}), BTreeMap::new(), "model", "stale");
    stale.status = ProposalStatus::Accepted;
    assert!(matches!(
        apply_proposal(base, stale, &policy),
        Err(ProposalError::StaleInputHash { .. })
    ));
}

#[test]
fn patch_accepts_only_workflow_and_provenance_keys() {
    let base = base_output();
    let mut unknown = accepted_proposal(&base, json!({"description": "not allowed"}));
    assert!(matches!(
        apply_proposal(base.clone(), unknown, &ProposalPolicy::from_base(&base)),
        Err(ProposalError::UnknownPatchKey { key }) if key == "description"
    ));

    unknown = accepted_proposal(
        &base,
        json!({"permissions": {"network": {"enabled": true}}}),
    );
    assert!(matches!(
        apply_proposal(base.clone(), unknown, &ProposalPolicy::from_base(&base)),
        Err(ProposalError::PermissionWidening)
    ));

    let malformed = accepted_proposal(&base, json!({"workflow": []}));
    assert!(matches!(
        apply_proposal(base.clone(), malformed, &ProposalPolicy::from_base(&base)),
        Err(ProposalError::MalformedWorkflow { .. })
    ));
}

#[test]
fn malformed_workflow_and_provenance_are_rejected() {
    let base = base_output();
    let policy = ProposalPolicy::from_base(&base);
    let unknown_action = accepted_proposal(
        &base,
        workflow_patch(
            &base,
            json!([{"action": "network_request", "id": "exec-0001"}]),
            Some(json!([source("exec-0001", &[0, 1])])),
        ),
    );
    assert!(matches!(
        apply_proposal(base.clone(), unknown_action, &policy),
        Err(ProposalError::MalformedWorkflow { .. })
    ));

    let unknown_field = accepted_proposal(
        &base,
        workflow_patch(
            &base,
            json!([{
                "action": "exec",
                "id": "exec-0001",
                "program": "python3",
                "args": [],
                "timeout_ms": 120000,
                "outputs": [],
                "env": {"TOKEN": "secret"}
            }]),
            Some(json!([source("exec-0001", &[0, 1])])),
        ),
    );
    assert!(matches!(
        apply_proposal(base.clone(), unknown_field, &policy),
        Err(ProposalError::MalformedWorkflow { .. })
    ));

    let malformed_provenance = accepted_proposal(
        &base,
        workflow_patch(
            &base,
            json!([replacement_step(&["-c", "print('done')"])]),
            Some(json!([{
                "step_id": "exec-0001",
                "event_sequences": [],
                "source_summary": "missing events"
            }])),
        ),
    );
    assert!(matches!(
        apply_proposal(base, malformed_provenance, &policy),
        Err(ProposalError::MalformedProvenance { .. })
    ));
}

#[test]
fn changed_or_added_steps_require_provenance() {
    let base = base_output();
    let policy = ProposalPolicy::from_base(&base);
    let added = json!({
        "action": "assert",
        "id": "assert-0002",
        "assertion": {
            "type": "file_exists",
            "path": "work/output.txt"
        }
    });
    let missing = accepted_proposal(
        &base,
        workflow_patch(
            &base,
            json!([replacement_step(&["-c", "print('done')"]), added]),
            None,
        ),
    );
    assert!(matches!(
        apply_proposal(base, missing, &policy),
        Err(ProposalError::MissingProvenance { step_id }) if step_id == "assert-0002"
    ));
}

#[test]
fn empty_patch_preserves_workflow_and_provenance() {
    let base = base_output();
    let before = base.clone();
    let proposal = accepted_proposal(&base, json!({}));
    let output = apply_proposal(base, proposal, &ProposalPolicy::from_base(&before))
        .expect("empty accepted patch should apply");

    assert_eq!(
        serde_json::to_value(&output.workflow).expect("workflow JSON"),
        serde_json::to_value(&before.workflow).expect("workflow JSON")
    );
    assert_eq!(output.provenance, before.provenance);
    assert_eq!(
        serde_json::to_value(&output.permissions).expect("permissions JSON"),
        serde_json::to_value(&before.permissions).expect("permissions JSON")
    );
}

#[test]
fn accepted_valid_application_is_deterministic_and_sorts_descriptions() {
    let base = base_output();
    let patch = workflow_patch(
        &base,
        json!([replacement_step(&["-c", "print('changed')"])]),
        Some(json!([source("exec-0001", &[0, 1])])),
    );
    let mut proposal = accepted_proposal(&base, patch);
    proposal
        .descriptions
        .insert("z-step".into(), "second description".into());
    proposal
        .descriptions
        .insert("a-step".into(), "first description".into());
    let policy = ProposalPolicy::from_base(&base);

    let first = apply_proposal(base.clone(), proposal.clone(), &policy)
        .expect("accepted valid proposal should apply");
    let second =
        apply_proposal(base, proposal, &policy).expect("same accepted proposal should apply twice");

    assert!(matches!(
        &first.workflow.steps[0],
        Step::Exec(step) if step.args == ["-c", "print('changed')"]
    ));
    assert_eq!(first.provenance[0].event_sequences, [0, 1]);
    assert!(first
        .skill_markdown
        .ends_with("- `z-step`: second description\n"));
    let a_position = first
        .skill_markdown
        .find("- `a-step`: first description")
        .expect("first description");
    let z_position = first
        .skill_markdown
        .find("- `z-step`: second description")
        .expect("second description");
    assert!(a_position < z_position);
    assert_eq!(
        first.deterministic_json().expect("first serialization"),
        second.deterministic_json().expect("second serialization")
    );
    assert_eq!(
        first.content_hash().expect("first hash"),
        second.content_hash().expect("second hash")
    );
}

#[test]
fn proposal_cannot_add_executables_or_network_or_secret_access() {
    let base = base_output();
    let policy = ProposalPolicy::from_base(&base);

    let new_executable = accepted_proposal(
        &base,
        workflow_patch(
            &base,
            json!([{
                "action": "exec",
                "id": "exec-0001",
                "program": "curl",
                "args": ["https://example.invalid"],
                "timeout_ms": 120000,
                "outputs": []
            }]),
            Some(json!([source("exec-0001", &[0, 1])])),
        ),
    );
    assert!(matches!(
        apply_proposal(base.clone(), new_executable, &policy),
        Err(ProposalError::UndeclaredExecutable { program, .. }) if program == "curl"
    ));

    let network = accepted_proposal(
        &base,
        workflow_patch(
            &base,
            json!([replacement_step(&[
                "-c",
                "download('https://example.invalid')"
            ])]),
            Some(json!([source("exec-0001", &[0, 1])])),
        ),
    );
    assert!(matches!(
        apply_proposal(base.clone(), network, &policy),
        Err(ProposalError::NetworkNotAllowed)
    ));

    let secret = accepted_proposal(
        &base,
        workflow_patch(
            &base,
            json!([replacement_step(&["-c", "print($SECRET)"])]),
            Some(json!([source("exec-0001", &[0, 1])])),
        ),
    );
    assert!(matches!(
        apply_proposal(base, secret, &policy),
        Err(ProposalError::SecretsNotAllowed)
    ));
}

#[test]
fn proposal_paths_must_be_safe_and_within_base_scopes() {
    let base = base_output();
    let policy = ProposalPolicy::from_base(&base);

    let unsafe_path = accepted_proposal(
        &base,
        workflow_patch(
            &base,
            json!([{
                "action": "script",
                "id": "script-0001",
                "path": "../outside.py",
                "args": [],
                "timeout_ms": 120000,
                "outputs": []
            }]),
            Some(json!([source("script-0001", &[0])])),
        ),
    );
    assert!(matches!(
        apply_proposal(base.clone(), unsafe_path, &policy),
        Err(ProposalError::UnsafePath { .. })
    ));

    let outside_scope = accepted_proposal(
        &base,
        workflow_patch(
            &base,
            json!([{
                "action": "assert",
                "id": "assert-0001",
                "assertion": {
                    "type": "file_exists",
                    "path": "secrets.txt"
                }
            }]),
            Some(json!([source("assert-0001", &[0])])),
        ),
    );
    assert!(matches!(
        apply_proposal(base, outside_scope, &policy),
        Err(ProposalError::PathOutsideBase { .. })
    ));
}

#[test]
fn provenance_event_sequences_must_come_from_the_base_policy() {
    let base = base_output();
    let proposal = accepted_proposal(
        &base,
        workflow_patch(
            &base,
            json!([replacement_step(&["-c", "print('changed')"])]),
            Some(json!([source("exec-0001", &[99])])),
        ),
    );

    assert!(matches!(
        apply_proposal(base.clone(), proposal, &ProposalPolicy::from_base(&base)),
        Err(ProposalError::EventSequenceOutsidePolicy {
            event_sequence: 99,
            ..
        })
    ));
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = std::task::Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut future = pin!(future);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("fake provider future should be immediately ready"),
    }
}
