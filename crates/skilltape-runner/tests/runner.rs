use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use skilltape_core::{create_skill_template, LoadedSkillPackage, SkillPackage};
use skilltape_policy::PolicyEngine;
use skilltape_runner::{
    run_skill_with_adapter, ProcessAdapter, ProcessError, ProcessOutput, ProcessRequest,
    ProcessStatus, ResourceLimits, RunEvent, RunRequest, RunStatus, StepStatus,
};
use skilltape_schema::{FileStep, Step};
use tempfile::{tempdir, TempDir};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

struct PackageFixture {
    _root: TempDir,
    package: LoadedSkillPackage,
}

type Snapshots = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

#[derive(Clone)]
enum FakeBehavior {
    Return(ProcessOutput),
    WaitForCancel,
    SpawnFailure,
}

#[derive(Clone, Default)]
struct FakeAdapter {
    behavior: Arc<Mutex<Option<FakeBehavior>>>,
    calls: Arc<AtomicUsize>,
    requests: Arc<Mutex<Vec<ProcessRequest>>>,
    snapshots: Snapshots,
}

impl FakeAdapter {
    fn new(behavior: FakeBehavior) -> Self {
        Self {
            behavior: Arc::new(Mutex::new(Some(behavior))),
            ..Self::default()
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn requests(&self) -> Vec<ProcessRequest> {
        self.requests.lock().expect("requests lock").clone()
    }

    fn snapshot(&self, relative: &str) -> Option<Vec<u8>> {
        self.snapshots
            .lock()
            .expect("snapshots lock")
            .iter()
            .find(|(path, _)| path == relative)
            .map(|(_, contents)| contents.clone())
    }
}

impl ProcessAdapter for FakeAdapter {
    fn run<'a>(
        &'a self,
        request: ProcessRequest,
        cancel: CancellationToken,
    ) -> skilltape_runner::ProcessFuture<'a> {
        let behavior = self
            .behavior
            .lock()
            .expect("behavior lock")
            .clone()
            .expect("fake behavior");
        self.calls.fetch_add(1, Ordering::SeqCst);
        for relative in ["inputs/fixture.txt", "scripts/emit.sh"] {
            if let Ok(contents) = fs::read(request.cwd.join(relative)) {
                self.snapshots
                    .lock()
                    .expect("snapshots lock")
                    .push((relative.to_owned(), contents));
            }
        }
        self.requests.lock().expect("requests lock").push(request);
        Box::pin(async move {
            match behavior {
                FakeBehavior::Return(output) => Ok(output),
                FakeBehavior::WaitForCancel => {
                    cancel.cancelled().await;
                    Ok(ProcessOutput::cancelled())
                }
                FakeBehavior::SpawnFailure => Err(ProcessError::SpawnFailed),
            }
        })
    }
}

fn package(workflow_steps: Vec<Value>, permissions: Value) -> PackageFixture {
    package_with_output(workflow_steps, permissions, None)
}

fn package_with_output(
    workflow_steps: Vec<Value>,
    permissions: Value,
    output_path: Option<&str>,
) -> PackageFixture {
    let root = tempdir().expect("package tempdir");
    let package_root = root.path().join("package");
    create_skill_template(&package_root, "runner-test").expect("template");
    if let Some(output_path) = output_path {
        let manifest = fs::read_to_string(package_root.join("skilltape.yaml")).expect("manifest");
        let output = format!("outputs:\n  - id: result\n    type: file\n    path: {output_path}\n");
        fs::write(
            package_root.join("skilltape.yaml"),
            manifest.replace("outputs: []\n", &output),
        )
        .expect("manifest output");
    }
    fs::write(
        package_root.join("workflow.yaml"),
        serde_json::to_vec(&json!({
            "schema": "skilltape.dev/workflow/v1",
            "steps": workflow_steps,
        }))
        .expect("workflow json"),
    )
    .expect("workflow");
    fs::write(
        package_root.join("permissions.json"),
        serde_json::to_vec(&permissions).expect("permissions json"),
    )
    .expect("permissions");
    let package = SkillPackage::load(&package_root).expect("loaded package");
    PackageFixture {
        _root: root,
        package,
    }
}

fn script_package(workflow_steps: Vec<Value>, permissions: Value) -> PackageFixture {
    let fixture = package(workflow_steps, permissions);
    let script_root = fixture.package.root.join("scripts");
    fs::create_dir_all(&script_root).expect("script root");
    fs::write(
        script_root.join("emit.sh"),
        b"#!/bin/sh\nprintf script-output\n",
    )
    .expect("script");
    fs::write(
        fixture.package.root.join("skilltape.lock"),
        serde_json::to_vec(&json!({
            "schema": "skilltape.dev/lock/v1",
            "engine": {"version": "0.1.0"},
            "tools": [],
            "scripts": [{"path": "scripts/emit.sh", "sha256": "fixture"}],
        }))
        .expect("lockfile json"),
    )
    .expect("lockfile");
    fixture
}

fn permissions(read: &[&str], write: &[&str], executables: &[&str]) -> Value {
    json!({
        "schema": "skilltape.dev/permissions/v1",
        "filesystem": {
            "read": read,
            "write": write,
        },
        "process": {
            "executables": executables,
            "max_processes": 2,
            "default_timeout_ms": 1000,
        },
        "network": {
            "enabled": false,
            "allow_hosts": [],
        },
        "secrets": {
            "read_environment": false,
        },
    })
}

fn exec_step(id: &str, program: &str, args: &[&str]) -> Value {
    json!({
        "action": "exec",
        "id": id,
        "program": program,
        "args": args,
        "timeout_ms": 1000,
    })
}

fn limits() -> ResourceLimits {
    ResourceLimits {
        max_processes: 2,
        step_timeout: Duration::from_secs(2),
        max_output_bytes: 64,
    }
}

async fn run(
    fixture: PackageFixture,
    input_root: &Path,
    output_root: &Path,
    adapter: &FakeAdapter,
    cancel: CancellationToken,
) -> (skilltape_runner::RunSummary, Vec<RunEvent>) {
    let (sender, mut receiver) = mpsc::channel(64);
    let summary = run_skill_with_adapter(
        RunRequest {
            package: fixture.package,
            input_root: input_root.to_owned(),
            output_root: output_root.to_owned(),
            limits: limits(),
        },
        PolicyEngine::default(),
        sender,
        cancel,
        adapter,
    )
    .await
    .expect("runner result");
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    (summary, events)
}

fn event_signature(events: &[RunEvent]) -> Vec<(u64, String, StepStatus, Option<i32>)> {
    events
        .iter()
        .map(|event| {
            (
                event.sequence,
                event.step_id.clone(),
                event.status,
                event.exit_code,
            )
        })
        .collect()
}

fn process_output(status: ProcessStatus, exit_code: Option<i32>, stdout: &str) -> ProcessOutput {
    process_output_with_stderr(status, exit_code, stdout, "")
}

fn process_output_with_stderr(
    status: ProcessStatus,
    exit_code: Option<i32>,
    stdout: &str,
    stderr: &str,
) -> ProcessOutput {
    ProcessOutput {
        status,
        exit_code,
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
        stdout_truncated: false,
        stderr_truncated: false,
    }
}

#[tokio::test]
async fn successful_exec_is_isolated_and_emits_deterministic_events() {
    let first = tempdir().expect("first root");
    let first_input = first.path().join("input");
    fs::create_dir(&first_input).expect("input");
    fs::write(first_input.join("fixture.txt"), "fixture").expect("fixture");
    let first_output = first.path().join("output");
    let first_fixture = package(
        vec![exec_step("print", "printf", &["hello"])],
        permissions(&["inputs/**"], &["outputs/**"], &["printf"]),
    );
    let first_adapter = FakeAdapter::new(FakeBehavior::Return(process_output(
        ProcessStatus::Exited,
        Some(0),
        "hello",
    )));
    let (first_summary, first_events) = run(
        first_fixture,
        &first_input,
        &first_output,
        &first_adapter,
        CancellationToken::new(),
    )
    .await;

    assert_eq!(first_summary.status, RunStatus::Succeeded);
    assert_eq!(first_adapter.calls(), 1);
    let request = &first_adapter.requests()[0];
    assert_ne!(request.cwd, first_input);
    assert!(!request.cwd.starts_with(first_output));
    assert_eq!(
        first_adapter.snapshot("inputs/fixture.txt"),
        Some(b"fixture".to_vec())
    );
    assert_eq!(
        event_signature(&first_events),
        vec![
            (0, "__run__".to_owned(), StepStatus::RunStarted, None),
            (1, "print".to_owned(), StepStatus::Started, None),
            (2, "print".to_owned(), StepStatus::Succeeded, Some(0)),
            (3, "__run__".to_owned(), StepStatus::RunSucceeded, None),
        ]
    );

    let second = tempdir().expect("second root");
    let second_input = second.path().join("input");
    fs::create_dir(&second_input).expect("input");
    fs::write(second_input.join("fixture.txt"), "fixture").expect("fixture");
    let second_output = second.path().join("output");
    let second_fixture = package(
        vec![exec_step("print", "printf", &["hello"])],
        permissions(&["inputs/**"], &["outputs/**"], &["printf"]),
    );
    let second_adapter = FakeAdapter::new(FakeBehavior::Return(process_output(
        ProcessStatus::Exited,
        Some(0),
        "hello",
    )));
    let (_, second_events) = run(
        second_fixture,
        &second_input,
        &second_output,
        &second_adapter,
        CancellationToken::new(),
    )
    .await;
    assert_eq!(
        event_signature(&first_events),
        event_signature(&second_events)
    );
}

#[tokio::test]
async fn non_zero_exit_is_a_failed_step_with_stable_status() {
    let root = tempdir().expect("root");
    let input = root.path().join("input");
    fs::create_dir(&input).expect("input");
    let output = root.path().join("output");
    let fixture = package(
        vec![exec_step("fail", "printf", &["bad"])],
        permissions(&[], &[], &["printf"]),
    );
    let adapter = FakeAdapter::new(FakeBehavior::Return(process_output(
        ProcessStatus::Exited,
        Some(7),
        "bad",
    )));
    let (summary, events) = run(fixture, &input, &output, &adapter, CancellationToken::new()).await;
    assert_eq!(summary.status, RunStatus::Failed);
    assert_eq!(adapter.calls(), 1);
    assert_eq!(events[2].status, StepStatus::Failed);
    assert_eq!(events[2].exit_code, Some(7));
}

#[tokio::test]
async fn timeout_and_spawn_failure_are_distinct() {
    let root = tempdir().expect("root");
    let input = root.path().join("input");
    fs::create_dir(&input).expect("input");
    let timeout_fixture = package(
        vec![exec_step("timeout", "printf", &["x"])],
        permissions(&[], &[], &["printf"]),
    );
    let timeout_adapter = FakeAdapter::new(FakeBehavior::Return(process_output(
        ProcessStatus::TimedOut,
        None,
        "",
    )));
    let (_, timeout_events) = run(
        timeout_fixture,
        &input,
        &root.path().join("timeout-output"),
        &timeout_adapter,
        CancellationToken::new(),
    )
    .await;
    assert_eq!(timeout_events[2].status, StepStatus::TimedOut);

    let spawn_fixture = package(
        vec![exec_step("spawn", "printf", &["x"])],
        permissions(&[], &[], &["printf"]),
    );
    let spawn_adapter = FakeAdapter::new(FakeBehavior::SpawnFailure);
    let (_, spawn_events) = run(
        spawn_fixture,
        &input,
        &root.path().join("spawn-output"),
        &spawn_adapter,
        CancellationToken::new(),
    )
    .await;
    assert_eq!(spawn_events[2].status, StepStatus::SpawnFailed);
}

#[tokio::test]
async fn cancellation_stops_a_waiting_process() {
    let root = tempdir().expect("root");
    let input = root.path().join("input");
    fs::create_dir(&input).expect("input");
    let fixture = package(
        vec![exec_step("cancel", "printf", &["x"])],
        permissions(&[], &[], &["printf"]),
    );
    let adapter = Arc::new(FakeAdapter::new(FakeBehavior::WaitForCancel));
    let cancel = CancellationToken::new();
    let (sender, mut receiver) = mpsc::channel(64);
    let task_adapter = adapter.clone();
    let task_cancel = cancel.clone();
    let task = tokio::spawn(async move {
        run_skill_with_adapter(
            RunRequest {
                package: fixture.package,
                input_root: input,
                output_root: root.path().join("output"),
                limits: limits(),
            },
            PolicyEngine::default(),
            sender,
            task_cancel,
            task_adapter.as_ref(),
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    cancel.cancel();
    let summary = task.await.expect("runner task").expect("runner result");
    while receiver.recv().await.is_some() {}
    assert_eq!(summary.status, RunStatus::Cancelled);
    assert_eq!(adapter.calls(), 1);
}

#[tokio::test]
async fn policy_denial_never_spawns_a_process() {
    let root = tempdir().expect("root");
    let input = root.path().join("input");
    fs::create_dir(&input).expect("input");
    let fixture = package(
        vec![exec_step("denied", "python", &["-c", "print(1)"])],
        permissions(&[], &[], &[]),
    );
    let adapter = FakeAdapter::new(FakeBehavior::Return(process_output(
        ProcessStatus::Exited,
        Some(0),
        "unexpected",
    )));
    let (summary, events) = run(
        fixture,
        &input,
        &root.path().join("output"),
        &adapter,
        CancellationToken::new(),
    )
    .await;
    assert_eq!(summary.status, RunStatus::Failed);
    assert_eq!(adapter.calls(), 0);
    assert_eq!(events[2].status, StepStatus::Denied);
}

#[tokio::test]
async fn output_is_truncated_to_the_declared_cap() {
    let root = tempdir().expect("root");
    let input = root.path().join("input");
    fs::create_dir(&input).expect("input");
    let fixture = package(
        vec![exec_step("output", "printf", &["x"])],
        permissions(&[], &[], &["printf"]),
    );
    let adapter = FakeAdapter::new(FakeBehavior::Return(process_output_with_stderr(
        ProcessStatus::Exited,
        Some(0),
        "0123456789abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz0123456789",
        "stderr-stderr-stderr-stderr-stderr-stderr-stderr-stderr-stderr-stderr-stderr-stderr",
    )));
    let (summary, events) = run(
        fixture,
        &input,
        &root.path().join("output"),
        &adapter,
        CancellationToken::new(),
    )
    .await;
    assert_eq!(summary.status, RunStatus::Succeeded);
    assert_eq!(events[2].stdout.len(), limits().max_output_bytes);
    assert_eq!(events[2].stderr.len(), limits().max_output_bytes);
    assert!(summary.output_truncated);
}

#[tokio::test]
async fn script_is_copied_into_the_isolated_workspace_before_spawn() {
    let root = tempdir().expect("root");
    let input = root.path().join("input");
    fs::create_dir(&input).expect("input");
    let fixture = script_package(
        vec![json!({
            "action": "script",
            "id": "script",
            "path": "scripts/emit.sh",
            "args": [],
            "timeout_ms": 1000,
        })],
        permissions(&["scripts/emit.sh"], &[], &[]),
    );
    let adapter = FakeAdapter::new(FakeBehavior::Return(process_output(
        ProcessStatus::Exited,
        Some(0),
        "script-output",
    )));

    let (summary, _) = run(
        fixture,
        &input,
        &root.path().join("output"),
        &adapter,
        CancellationToken::new(),
    )
    .await;

    assert_eq!(summary.status, RunStatus::Succeeded);
    assert_eq!(
        adapter.snapshot("scripts/emit.sh"),
        Some(b"#!/bin/sh\nprintf script-output\n".to_vec())
    );
}

#[tokio::test]
async fn file_move_assert_and_materialization_are_policy_guarded() {
    let root = tempdir().expect("root");
    let input = root.path().join("input");
    fs::create_dir(&input).expect("input");
    fs::write(input.join("source.txt"), "payload").expect("source");
    let output = root.path().join("output");
    let steps = vec![
        json!({
            "action": "file",
            "id": "move",
            "operation": "move",
            "from": "inputs/source.txt",
            "to": "outputs/result.txt",
        }),
        json!({
            "action": "assert",
            "id": "exists",
            "assertion": {"type": "file_exists", "path": "outputs/result.txt"},
        }),
    ];
    let fixture = package_with_output(
        steps,
        permissions(&["inputs/**", "outputs/**"], &["outputs/**"], &[]),
        Some("outputs/result.txt"),
    );
    let adapter = FakeAdapter::default();
    let (summary, events) = run(fixture, &input, &output, &adapter, CancellationToken::new()).await;
    assert_eq!(summary.status, RunStatus::Succeeded);
    assert_eq!(adapter.calls(), 0);
    assert_eq!(
        events
            .iter()
            .filter(|event| event.status == StepStatus::Denied)
            .count(),
        0
    );
    assert_eq!(
        fs::read_to_string(output.join("result.txt")).unwrap(),
        "payload"
    );
}

#[tokio::test]
async fn assertions_support_absent_and_sha256_and_only_materialize_on_success() {
    let root = tempdir().expect("root");
    let input = root.path().join("input");
    fs::create_dir(&input).expect("input");
    fs::write(input.join("source.txt"), "payload").expect("source");
    let output = root.path().join("output");
    let fixture = package_with_output(
        vec![
            json!({
                "action": "file",
                "id": "copy",
                "operation": "copy",
                "from": "inputs/source.txt",
                "to": "outputs/result.txt",
            }),
            json!({
                "action": "assert",
                "id": "hash",
                "assertion": {
                    "type": "file_hash",
                    "path": "outputs/result.txt",
                    "hash": "239f59ed55e737c77147cf55ad0c1b030b6d7ee748a7426952f9b852d5a935e5",
                },
            }),
            json!({
                "action": "assert",
                "id": "absent",
                "assertion": {"type": "file_absent", "path": "outputs/missing.txt"},
            }),
        ],
        permissions(&["inputs/**", "outputs/**"], &["outputs/**"], &[]),
        Some("outputs/result.txt"),
    );
    let adapter = FakeAdapter::default();
    let (summary, _) = run(fixture, &input, &output, &adapter, CancellationToken::new()).await;

    assert_eq!(summary.status, RunStatus::Succeeded);
    assert_eq!(
        fs::read_to_string(output.join("result.txt")).unwrap(),
        "payload"
    );

    let failed_output = root.path().join("failed-output");
    let failed_fixture = package_with_output(
        vec![exec_step("fail", "printf", &["failed"])],
        permissions(&[], &["outputs/**"], &["printf"]),
        Some("outputs/result.txt"),
    );
    let failed_adapter = FakeAdapter::new(FakeBehavior::Return(process_output(
        ProcessStatus::Exited,
        Some(9),
        "failed",
    )));
    let (failed_summary, _) = run(
        failed_fixture,
        &input,
        &failed_output,
        &failed_adapter,
        CancellationToken::new(),
    )
    .await;
    assert_eq!(failed_summary.status, RunStatus::Failed);
    assert!(!failed_output.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn rejects_symlinked_input_root() {
    use std::os::unix::fs::symlink;

    let root = tempdir().expect("root");
    let real_input = root.path().join("real-input");
    fs::create_dir(&real_input).expect("real input");
    let input = root.path().join("input-link");
    symlink(&real_input, &input).expect("input symlink");
    let fixture = package(
        vec![exec_step("print", "printf", &["x"])],
        permissions(&[], &[], &["printf"]),
    );
    let (sender, _receiver) = mpsc::channel(4);
    let result = run_skill_with_adapter(
        RunRequest {
            package: fixture.package,
            input_root: input,
            output_root: root.path().join("output"),
            limits: limits(),
        },
        PolicyEngine::default(),
        sender,
        CancellationToken::new(),
        &FakeAdapter::new(FakeBehavior::SpawnFailure),
    )
    .await;
    assert!(matches!(
        result,
        Err(skilltape_runner::RunError::InvalidInputRoot { .. })
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn real_printf_uses_isolated_process_environment() {
    if !Path::new("/usr/bin/printf").exists() {
        return;
    }
    let root = tempdir().expect("root");
    let input = root.path().join("input");
    fs::create_dir(&input).expect("input");
    let fixture = package(
        vec![exec_step("real", "/usr/bin/printf", &["hello\n"])],
        permissions(&[], &[], &["/usr/bin/printf"]),
    );
    let (sender, mut receiver) = mpsc::channel(16);
    let summary = skilltape_runner::run_skill(
        RunRequest {
            package: fixture.package,
            input_root: input,
            output_root: root.path().join("output"),
            limits: limits(),
        },
        PolicyEngine::default(),
        sender,
        CancellationToken::new(),
    )
    .await
    .expect("real printf");
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    assert_eq!(summary.status, RunStatus::Succeeded);
    assert_eq!(events[2].stdout, "hello\n");
}

#[test]
fn file_step_shape_remains_the_schema_contract() {
    let value = serde_json::from_value::<Step>(json!({
        "action": "file",
        "id": "move",
        "operation": "move",
        "from": "a",
        "to": "b",
    }))
    .expect("file step");
    assert!(matches!(value, Step::File(FileStep { operation, .. }) if operation == "move"));
}
