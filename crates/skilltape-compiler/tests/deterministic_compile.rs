use std::fs;

use serde_json::json;
use skilltape_compiler::{
    CompileError, CompileRequest, CompileTarget, Compiler, DeterministicCompiler, TapeSession,
};
use skilltape_core::SkillPackage;
use skilltape_schema::{SchemaId, Step};
use skilltape_tape::{EventSource, RedactionState, TapeEvent, TapeEventKind};
use tempfile::TempDir;

fn event(
    sequence: u64,
    kind: TapeEventKind,
    source: EventSource,
    payload: serde_json::Value,
) -> TapeEvent {
    TapeEvent {
        sequence,
        occurred_at_ms: 1_000 + sequence,
        kind,
        source,
        payload,
        redaction: RedactionState::Redacted,
    }
}

fn request(events: Vec<TapeEvent>, name: &str) -> CompileRequest {
    CompileRequest::new(
        TapeSession::new(events).expect("test tape should be ordered"),
        name,
        CompileTarget::new("generic-agent-skill", "0.1.0").expect("test target"),
    )
    .expect("test request")
}

fn terminal_tape() -> Vec<TapeEvent> {
    vec![
        event(
            0,
            TapeEventKind::SessionStarted,
            EventSource::System,
            json!({"cwd": "workspace", "environment": []}),
        ),
        event(
            1,
            TapeEventKind::TerminalCommand,
            EventSource::Shell,
            json!({
                "phase": "started",
                "command": "python3",
                "args": ["-c", "print('done')"],
                "cwd": "work"
            }),
        ),
        event(
            2,
            TapeEventKind::TerminalCommand,
            EventSource::Shell,
            json!({
                "phase": "output",
                "stream": "pty",
                "stdout_stderr_merged": true,
                "text": "done\napi_key=super-secret-value\n",
                "original_bytes": 33,
                "truncated": false,
                "redactions": [{
                    "name": "api_key",
                    "original_bytes": 16,
                    "sha256": "redacted-hash"
                }]
            }),
        ),
        event(
            3,
            TapeEventKind::SessionFinished,
            EventSource::System,
            json!({"exit_code": 0, "signal": null}),
        ),
    ]
}

#[test]
fn terminal_events_compile_to_one_exec_with_scoped_permissions() {
    let compiler = DeterministicCompiler;
    let first = compiler
        .compile(request(terminal_tape(), "terminal-skill"))
        .expect("terminal tape should compile");
    let second = compiler
        .compile(request(terminal_tape(), "terminal-skill"))
        .expect("same terminal tape should compile twice");

    assert_eq!(first.workflow.schema, SchemaId::WorkflowV1.uri());
    assert_eq!(first.workflow.steps.len(), 1);
    match &first.workflow.steps[0] {
        Step::Exec(step) => {
            assert_eq!(step.id, "exec-0001");
            assert_eq!(step.program, "python3");
            assert_eq!(step.args, ["-c", "print('done')"]);
            assert_eq!(step.timeout_ms, 120_000);
            assert!(step.outputs.is_empty());
        }
        other => panic!("expected one exec step, got {other:?}"),
    }
    assert_eq!(
        first.permissions.filesystem.read,
        ["work/**"],
        "the terminal cwd is the only inferred filesystem read scope"
    );
    assert!(first.permissions.filesystem.write.is_empty());
    assert_eq!(first.permissions.process.executables, ["python3"]);
    assert_eq!(first.permissions.process.max_processes, 1);
    assert_eq!(first.permissions.process.default_timeout_ms, 120_000);
    assert!(!first.permissions.network.enabled);
    assert!(first.permissions.network.allow_hosts.is_empty());
    assert!(!first.permissions.secrets.read_environment);
    assert_eq!(first.provenance.len(), 1);
    assert_eq!(first.provenance[0].step_id, "exec-0001");
    assert_eq!(first.provenance[0].event_sequences, [1, 2]);
    assert!(!first.provenance[0].source_summary.is_empty());

    assert_eq!(
        first.deterministic_json().expect("serialize first output"),
        second
            .deterministic_json()
            .expect("serialize second output")
    );
    assert_eq!(
        first.content_hash().expect("hash first output"),
        second.content_hash().expect("hash second output")
    );

    let serialized = String::from_utf8(first.deterministic_json().expect("serialize output"))
        .expect("deterministic JSON is UTF-8");
    assert!(!serialized.contains("super-secret-value"));
    assert!(!serialized.contains("done\\n"));
}

#[test]
fn filesystem_events_deduplicate_paths_and_preserve_metadata_only() {
    let tape = vec![
        event(
            0,
            TapeEventKind::FilesystemChanged,
            EventSource::Filesystem,
            json!({
                "kind": "created",
                "path": "src/main.rs",
                "content_hash": "hash-before",
                "size": 12
            }),
        ),
        event(
            1,
            TapeEventKind::FilesystemChanged,
            EventSource::Filesystem,
            json!({
                "kind": "modified",
                "path": "src/main.rs",
                "content_hash": "hash-after",
                "size": 18
            }),
        ),
        event(
            2,
            TapeEventKind::FilesystemChanged,
            EventSource::Filesystem,
            json!({
                "kind": "moved",
                "path": "src/new.rs",
                "previous_path": "src/old.rs",
                "content_hash": "hash-moved",
                "size": 18
            }),
        ),
        event(
            3,
            TapeEventKind::FilesystemChanged,
            EventSource::Filesystem,
            json!({"kind": "deleted", "path": "scratch.tmp"}),
        ),
    ];

    let output = DeterministicCompiler
        .compile(request(tape, "filesystem-skill"))
        .expect("filesystem tape should compile");

    assert_eq!(output.workflow.schema, SchemaId::WorkflowV1.uri());
    assert_eq!(output.workflow.steps.len(), 3);
    assert!(matches!(
        &output.workflow.steps[0],
        Step::Assert(step)
            if step.id == "assert-0001"
                && step.assertion.assertion_type == "file_hash"
                && step.assertion.path.as_deref() == Some("src/main.rs")
                && step.assertion.hash.as_deref() == Some("hash-after")
    ));
    assert!(matches!(
        &output.workflow.steps[1],
        Step::File(step)
            if step.id == "file-0002"
                && step.operation == "move"
                && step.from_path == "src/old.rs"
                && step.to_path == "src/new.rs"
    ));
    assert!(matches!(
        &output.workflow.steps[2],
        Step::Assert(step)
            if step.id == "assert-0003"
                && step.assertion.assertion_type == "file_absent"
                && step.assertion.path.as_deref() == Some("scratch.tmp")
    ));
    assert_eq!(
        output
            .provenance
            .iter()
            .map(|source| source.event_sequences.clone())
            .collect::<Vec<_>>(),
        vec![vec![0, 1], vec![2], vec![3]]
    );
    assert_eq!(
        output.permissions.filesystem.read,
        ["scratch.tmp", "src/main.rs", "src/old.rs"]
    );
    assert_eq!(
        output.permissions.filesystem.write,
        ["src/new.rs"],
        "move destinations are the only inferred filesystem writes"
    );
    assert!(output.permissions.process.executables.is_empty());
    assert!(!output.permissions.network.enabled);

    let metadata = output
        .fixtures
        .files
        .values()
        .find(|contents| contents.contains("hash-after"))
        .expect("deduplicated metadata fixture");
    assert!(metadata.contains("src/main.rs"));
    assert!(!metadata.contains("file contents"));
    assert!(!metadata.contains("super-secret-value"));
    assert_eq!(
        output
            .fixtures
            .files
            .keys()
            .filter(|path| path.starts_with("fixtures/changes/"))
            .count(),
        3,
        "one metadata fixture per emitted file behavior"
    );
}

#[test]
fn move_then_modified_destination_keeps_move_step_provenance_and_permissions() {
    let tape = vec![
        event(
            0,
            TapeEventKind::FilesystemChanged,
            EventSource::Filesystem,
            json!({
                "kind": "moved",
                "path": "src/new.rs",
                "previous_path": "src/old.rs",
                "content_hash": "hash-moved",
                "size": 18
            }),
        ),
        event(
            1,
            TapeEventKind::FilesystemChanged,
            EventSource::Filesystem,
            json!({
                "kind": "modified",
                "path": "src/new.rs",
                "content_hash": "hash-after",
                "size": 24
            }),
        ),
    ];

    let output = DeterministicCompiler
        .compile(request(tape, "move-modify-skill"))
        .expect("move then modify tape should compile");

    assert_eq!(output.workflow.steps.len(), 2);
    assert!(matches!(
        &output.workflow.steps[0],
        Step::File(step)
            if step.id == "file-0001"
                && step.operation == "move"
                && step.from_path == "src/old.rs"
                && step.to_path == "src/new.rs"
    ));
    assert!(matches!(
        &output.workflow.steps[1],
        Step::Assert(step)
            if step.id == "assert-0002"
                && step.assertion.assertion_type == "file_hash"
                && step.assertion.path.as_deref() == Some("src/new.rs")
                && step.assertion.hash.as_deref() == Some("hash-after")
    ));
    assert_eq!(
        output
            .provenance
            .iter()
            .map(|source| source.event_sequences.clone())
            .collect::<Vec<_>>(),
        vec![vec![0], vec![1]]
    );
    assert_eq!(output.provenance[0].step_id, "file-0001");
    assert_eq!(
        output.provenance[0].source_summary,
        "filesystem moved `src/old.rs` to `src/new.rs`"
    );
    assert_eq!(output.provenance[1].step_id, "assert-0002");
    assert_eq!(
        output.provenance[1].source_summary,
        "filesystem modified `src/new.rs`"
    );
    assert_eq!(
        output.permissions.filesystem.read,
        ["src/new.rs", "src/old.rs"]
    );
    assert_eq!(output.permissions.filesystem.write, ["src/new.rs"]);
}

#[test]
fn generated_package_support_files_load_and_lint_cleanly() {
    let output = DeterministicCompiler
        .compile(request(terminal_tape(), "lintable-skill"))
        .expect("terminal tape should compile");
    let temporary = TempDir::new().expect("temporary package parent");
    let root = temporary.path().join("package");

    fs::create_dir(&root).expect("package root");
    for (path, contents) in &output.fixtures.files {
        write_package_file(&root, path, contents);
    }
    write_package_file(&root, "SKILL.md", &output.skill_markdown);
    write_package_file(
        &root,
        "workflow.yaml",
        &serde_json::to_string_pretty(&output.workflow).expect("workflow JSON"),
    );
    write_package_file(
        &root,
        "permissions.json",
        &serde_json::to_string_pretty(&output.permissions).expect("permissions JSON"),
    );

    assert!(output.fixtures.files.contains_key("skilltape.yaml"));
    assert!(output.fixtures.files.contains_key("skilltape.lock"));
    assert!(output.fixtures.files.contains_key("README.md"));
    let loaded = SkillPackage::load(&root).expect("generated package should load");
    let report = loaded.lint(false);
    assert!(
        report.errors.is_empty(),
        "unexpected errors: {:?}",
        report.errors
    );
    assert!(
        report.warnings.is_empty(),
        "unexpected warnings: {:?}",
        report.warnings
    );
}

#[test]
fn malformed_and_unsafe_payloads_return_typed_errors() {
    let missing_command = request(
        vec![event(
            0,
            TapeEventKind::TerminalCommand,
            EventSource::Shell,
            json!({"phase": "started", "args": [], "cwd": "work"}),
        )],
        "malformed-skill",
    );
    assert!(matches!(
        DeterministicCompiler.compile(missing_command),
        Err(CompileError::MalformedPayload {
            sequence: 0,
            ref field,
            ..
        }) if field == "command"
    ));

    let unsafe_path = request(
        vec![event(
            0,
            TapeEventKind::FilesystemChanged,
            EventSource::Filesystem,
            json!({"kind": "created", "path": "../outside.txt", "size": 1}),
        )],
        "unsafe-skill",
    );
    assert!(matches!(
        DeterministicCompiler.compile(unsafe_path),
        Err(CompileError::UnsafePath { sequence: 0, ref path }) if path == "../outside.txt"
    ));

    let unsupported_kind = request(
        vec![event(
            0,
            TapeEventKind::FilesystemChanged,
            EventSource::Filesystem,
            json!({"kind": "replaced", "path": "file.txt"}),
        )],
        "unsupported-skill",
    );
    assert!(matches!(
        DeterministicCompiler.compile(unsupported_kind),
        Err(CompileError::UnsupportedPayload { sequence: 0, .. })
    ));
}

#[test]
fn unpaired_terminal_output_is_an_ambiguous_grouping_error() {
    let output_only = request(
        vec![event(
            0,
            TapeEventKind::TerminalCommand,
            EventSource::Shell,
            json!({"phase": "output", "text": "ignored"}),
        )],
        "ambiguous-skill",
    );

    assert!(matches!(
        DeterministicCompiler.compile(output_only),
        Err(CompileError::AmbiguousTerminalGrouping { sequence: 0, .. })
    ));
}

fn write_package_file(root: &std::path::Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("package parent");
    }
    fs::write(path, contents).expect("package file");
}

#[test]
fn captured_outputs_are_declared_for_replay_and_delivery() {
    let mut events = terminal_tape();
    let next = events.last().unwrap().sequence + 1;
    events.push(event(next, TapeEventKind::FilesystemChanged, EventSource::Filesystem,
        json!({"kind":"created","path":"outputs/report.json","content_hash":"ab".repeat(32),"size":12})));
    let artifact = DeterministicCompiler
        .compile(request(events, "output-test"))
        .unwrap();
    let Step::Exec(step) = &artifact.workflow.steps[0] else {
        panic!("exec")
    };
    assert_eq!(step.outputs.len(), 1);
    assert_eq!(step.outputs[0].path, "outputs/report.json");
    assert!(artifact
        .permissions
        .filesystem
        .write
        .contains(&"outputs/report.json".into()));
    assert!(!artifact
        .permissions
        .filesystem
        .write
        .contains(&"inputs/**".into()));
}

#[test]
fn compiler_input_rejects_recorded_interruption_and_invalid_completion_flags() {
    for completion in [
        json!({"cancelled": true}),
        json!({"timed_out": true}),
        json!({"error": "synthetic-private-interruption"}),
        json!({"cancelled": "false"}),
        json!({"timed_out": 0}),
    ] {
        let mut events = terminal_tape();
        events.last_mut().unwrap().payload = completion;
        assert!(
            TapeSession::new(events.clone()).is_err(),
            "interrupted recording must not become compiler input"
        );
        assert!(
            serde_json::from_value::<TapeSession>(json!({"events": events})).is_err(),
            "deserialization must use the same completion guard"
        );
    }
}

#[test]
fn filesystem_observed_during_a_command_follows_its_exec_and_keeps_all_sources() {
    let mut tape = terminal_tape();
    for event in &mut tape[2..] {
        event.sequence += 1;
        event.occurred_at_ms += 1;
    }
    tape.insert(2,event(2,TapeEventKind::FilesystemChanged,EventSource::Filesystem,json!({"kind":"created","path":"outputs/result.txt","size":1,"content_hash":"a".repeat(64)})));
    let artifact = DeterministicCompiler
        .compile(request(tape, "interleaved"))
        .expect("a completed command may contain filesystem observations");
    assert_eq!(artifact.workflow.steps.len(), 2);
    let Step::Exec(exec) = &artifact.workflow.steps[0] else {
        panic!("command must precede its output assertion")
    };
    assert_eq!(exec.outputs[0].path, "outputs/result.txt");
    assert!(matches!(&artifact.workflow.steps[1], Step::Assert(_)));
    assert_eq!(artifact.provenance[0].event_sequences, vec![1, 3]);
    assert_eq!(artifact.provenance[1].event_sequences, vec![2]);
}

#[test]
fn later_commands_do_not_coalesce_an_earlier_output_assertion() {
    let mut tape = Vec::new();
    for (start, hash) in [(0, "a".repeat(64)), (3, "b".repeat(64))] {
        tape.push(event(
            start,
            TapeEventKind::TerminalCommand,
            EventSource::Shell,
            json!({"phase":"started","command":"synthetic-tool","args":[],"cwd":"workspace"}),
        ));
        tape.push(event(
            start + 1,
            TapeEventKind::FilesystemChanged,
            EventSource::Filesystem,
            json!({"kind":"modified","path":"outputs/result.txt","size":1,"content_hash":hash}),
        ));
        tape.push(event(
            start + 2,
            TapeEventKind::TerminalCommand,
            EventSource::Shell,
            json!({"phase":"output","text":""}),
        ));
    }
    let artifact = DeterministicCompiler
        .compile(request(tape, "two-command"))
        .unwrap();
    assert_eq!(artifact.workflow.steps.len(), 4);
    let Step::Assert(first) = &artifact.workflow.steps[1] else {
        panic!("first assertion")
    };
    let Step::Assert(second) = &artifact.workflow.steps[3] else {
        panic!("second assertion")
    };
    assert_eq!(first.assertion.hash, Some("a".repeat(64)));
    assert_eq!(second.assertion.hash, Some("b".repeat(64)));
    assert_eq!(artifact.provenance[1].event_sequences, vec![1]);
    assert_eq!(artifact.provenance[3].event_sequences, vec![4]);
}
