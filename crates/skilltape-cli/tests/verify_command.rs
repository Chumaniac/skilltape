use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::{json, Value};
use skilltape_core::create_skill_template;
use skilltape_tape::{TapeManifest, TapeStore, TAPE_SCHEMA_V1};
use tempfile::TempDir;

fn permissions(read: &[&str], write: &[&str], executables: &[&str]) -> Value {
    json!({
        "schema": "skilltape.dev/permissions/v1",
        "filesystem": {"read": read, "write": write},
        "process": {
            "executables": executables,
            "max_processes": 1,
            "default_timeout_ms": 1000
        },
        "network": {"enabled": false, "allow_hosts": []},
        "secrets": {"read_environment": false}
    })
}

fn write_package(root: &Path, steps: Value, permission_document: Value) {
    create_skill_template(root, "cli-test").expect("skill template");
    fs::write(
        root.join("workflow.yaml"),
        serde_json::to_vec(&json!({
            "schema": "skilltape.dev/workflow/v1",
            "steps": steps,
        }))
        .expect("workflow JSON"),
    )
    .expect("workflow");
    fs::write(
        root.join("permissions.json"),
        serde_json::to_vec(&permission_document).expect("permissions JSON"),
    )
    .expect("permissions");
}

fn file_package() -> (TempDir, PathBuf, PathBuf) {
    let temp = TempDir::new().expect("temp directory");
    let skill = temp.path().join("skill");
    let input = temp.path().join("input");
    fs::create_dir(&input).expect("input");
    fs::write(input.join("source.txt"), "secret-cli-value").expect("source");
    write_package(
        &skill,
        json!([{
            "action": "file",
            "id": "copy",
            "operation": "copy",
            "from": "inputs/source.txt",
            "to": "outputs/result.txt"
        }]),
        permissions(&["inputs/**", "outputs/**"], &["outputs/**"], &[]),
    );
    (temp, skill, input)
}

fn empty_tape(path: &Path) {
    let store = TapeStore::create(
        path,
        TapeManifest {
            schema: TAPE_SCHEMA_V1.to_owned(),
            id: "journey".to_owned(),
            started_at_ms: 1,
            finished_at_ms: None,
            platform: "test".to_owned(),
            workspace_root: "workspace".to_owned(),
            event_count: 0,
        },
    )
    .expect("tape");
    store.finish(2).expect("finish tape");
}

#[test]
fn replay_clean_package_emits_only_redacted_json_summary() {
    let (_temp, skill, input) = file_package();
    let output = Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["replay"])
        .arg(&skill)
        .args(["--input"])
        .arg(&input)
        .arg("--json")
        .assert()
        .success()
        .stderr(predicates::str::is_empty())
        .get_output()
        .stdout
        .clone();

    let document: Value = serde_json::from_slice(&output).expect("replay JSON");
    assert_eq!(document["schema"], "skilltape.dev/replay/v1");
    assert_eq!(document["status"], "succeeded");
    assert!(!String::from_utf8_lossy(&output).contains("secret-cli-value"));
    assert!(document["steps"]
        .as_array()
        .is_some_and(|steps| !steps.is_empty()));
}

#[test]
fn verify_clean_package_writes_a_receipt_and_emits_the_same_document() {
    let (temp, skill, input) = file_package();
    let receipt_path = temp.path().join("receipts/run.json");
    let output = Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["verify"])
        .arg(&skill)
        .args(["--input"])
        .arg(&input)
        .args(["--receipt"])
        .arg(&receipt_path)
        .arg("--json")
        .assert()
        .success()
        .stderr(predicates::str::is_empty())
        .get_output()
        .stdout
        .clone();

    let stdout: Value = serde_json::from_slice(&output).expect("receipt JSON");
    let written: Value = serde_json::from_slice(&fs::read(&receipt_path).expect("receipt file"))
        .expect("written receipt JSON");
    assert_eq!(stdout, written);
    assert_eq!(stdout["schema"], "skilltape.dev/receipt/v1");
    assert_eq!(stdout["status"], "succeeded");
    assert!(!String::from_utf8_lossy(&output).contains("secret-cli-value"));
}

#[test]
fn verify_workflow_assertion_failure_returns_three_and_writes_failed_receipt() {
    let (temp, skill, input) = file_package();
    fs::write(
        skill.join("workflow.yaml"),
        serde_json::to_vec(&json!({
            "schema": "skilltape.dev/workflow/v1",
            "steps": [
                {
                    "action": "file",
                    "id": "copy",
                    "operation": "copy",
                    "from": "inputs/source.txt",
                    "to": "outputs/result.txt"
                },
                {
                    "action": "assert",
                    "id": "hash",
                    "assertion": {
                        "type": "file_hash",
                        "path": "outputs/result.txt",
                        "hash": "0000000000000000000000000000000000000000000000000000000000000000"
                    }
                }
            ]
        }))
        .expect("workflow JSON"),
    )
    .expect("workflow");
    let receipt_path = temp.path().join("failed.json");

    let output = Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["verify"])
        .arg(&skill)
        .args(["--input"])
        .arg(&input)
        .args(["--receipt"])
        .arg(&receipt_path)
        .arg("--json")
        .assert()
        .code(3)
        .stderr(predicates::str::is_empty())
        .get_output()
        .stdout
        .clone();

    let receipt: Value = serde_json::from_slice(&output).expect("failed receipt JSON");
    assert_eq!(receipt["status"], "run_failed");
    assert_eq!(receipt["assertions"].as_array().map(Vec::len), Some(0));
    assert!(receipt["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .any(|step| step["status"] == "failed"));
    assert!(receipt_path.is_file());
}

#[test]
fn replay_policy_rejection_returns_three_without_process_output() {
    let temp = TempDir::new().expect("temp directory");
    let skill = temp.path().join("skill");
    write_package(
        &skill,
        json!([{
            "action": "exec",
            "id": "blocked",
            "program": "/usr/bin/printf",
            "args": ["secret-cli-value"],
            "timeout_ms": 1000
        }]),
        permissions(&[], &[], &[]),
    );

    let output = Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["replay"])
        .arg(&skill)
        .arg("--json")
        .assert()
        .code(3)
        .stderr(predicates::str::is_empty())
        .get_output()
        .stdout
        .clone();
    let document: Value = serde_json::from_slice(&output).expect("replay JSON");
    assert_eq!(document["status"], "failed");
    assert_eq!(document["failure"]["kind"], "policy_denied");
    assert!(!String::from_utf8_lossy(&output).contains("secret-cli-value"));
    assert!(document["policy_decisions"]
        .as_array()
        .expect("policy decisions")
        .iter()
        .any(|decision| decision["allowed"] == false));
}

#[test]
fn verify_input_error_goes_to_stderr_and_json_stdout_stays_empty() {
    let temp = TempDir::new().expect("temp directory");
    let missing = temp.path().join("missing-skill");

    Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["verify"])
        .arg(missing)
        .arg("--json")
        .assert()
        .code(2)
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::contains("skill package failed to load"));
}

#[test]
fn verify_receipt_does_not_overwrite_existing_file() {
    let (temp, skill, input) = file_package();
    let receipt_path = temp.path().join("receipt.json");
    Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["verify"])
        .arg(&skill)
        .args(["--input"])
        .arg(&input)
        .args(["--receipt"])
        .arg(&receipt_path)
        .assert()
        .success();
    let original = fs::read(&receipt_path).expect("receipt");

    Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["verify"])
        .arg(&skill)
        .args(["--input"])
        .arg(&input)
        .args(["--receipt"])
        .arg(&receipt_path)
        .assert()
        .code(2)
        .stderr(predicates::str::contains("already exists"));

    assert_eq!(fs::read(receipt_path).expect("receipt remains"), original);
}

#[cfg(unix)]
#[test]
fn verify_receipt_rejects_a_symlinked_parent() {
    let (temp, skill, input) = file_package();
    let outside = temp.path().join("outside");
    let linked = temp.path().join("linked");
    fs::create_dir(&outside).expect("outside");
    std::os::unix::fs::symlink(&outside, &linked).expect("symlink");

    Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["verify"])
        .arg(&skill)
        .args(["--input"])
        .arg(&input)
        .args(["--receipt"])
        .arg(linked.join("receipt.json"))
        .assert()
        .code(2)
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::contains("unsafe"));

    assert!(!outside.join("receipt.json").exists());
}

#[cfg(target_os = "macos")]
#[test]
fn replay_timeout_returns_runtime_failure() {
    let temp = TempDir::new().expect("temp directory");
    let skill = temp.path().join("skill");
    write_package(
        &skill,
        json!([{
            "action": "exec",
            "id": "sleep",
            "program": "/usr/bin/sleep",
            "args": ["2"],
            "timeout_ms": 1
        }]),
        permissions(&[], &[], &["/usr/bin/sleep"]),
    );

    let output = Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["replay"])
        .arg(&skill)
        .arg("--json")
        .assert()
        .code(4)
        .stderr(predicates::str::is_empty())
        .get_output()
        .stdout
        .clone();
    let document: Value = serde_json::from_slice(&output).expect("timeout JSON");
    assert_eq!(document["status"], "failed");
    assert!(document["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .any(|step| step["status"] == "timed_out"));
}

#[test]
fn init_compile_verify_journey_produces_a_schema_receipt() {
    let temp = TempDir::new().expect("temp directory");
    let initialized = temp.path().join("initialized");
    let tape = temp.path().join("empty-tape");
    let compiled = temp.path().join("compiled");
    Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["init", "journey", "--output"])
        .arg(&initialized)
        .assert()
        .success();
    empty_tape(&tape);
    Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["compile"])
        .arg(&tape)
        .args(["--output"])
        .arg(&compiled)
        .assert()
        .success();

    let output = Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["verify"])
        .arg(&compiled)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let receipt: Value = serde_json::from_slice(&output).expect("journey receipt");
    assert_eq!(receipt["schema"], "skilltape.dev/receipt/v1");
    assert_eq!(receipt["status"], "succeeded");
}

#[cfg(unix)]
fn interrupt_running_command(command_name: &str, delivery_mode: bool) {
    use assert_cmd::cargo::CommandCargoExt;
    use std::process::{Command as ProcessCommand, Stdio};
    use std::time::{Duration, Instant};
    let temp = TempDir::new().expect("temp");
    let skill = temp.path().join("skill");
    let scratch = temp.path().join("scratch");
    fs::create_dir(&scratch).expect("scratch");
    let mut policy = permissions(&[], &["outputs/**"], &["/bin/sleep"]);
    policy["process"]["default_timeout_ms"] = json!(10_000);
    write_package(
        &skill,
        json!([{
            "action": "exec", "id": "waiting", "program": "/bin/sleep",
            "args": ["4"], "timeout_ms": 10_000
        }]),
        policy,
    );
    let receipt_path = temp.path().join("receipt.json");
    let delivery_path = temp.path().join("delivery");
    let mut command = ProcessCommand::cargo_bin("skilltape").expect("binary");
    command
        .args([command_name])
        .arg(&skill)
        .arg("--json")
        .env("TMPDIR", &scratch)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if command_name == "verify" {
        if delivery_mode {
            command.arg("--delivery-dir").arg(&delivery_path);
        } else {
            command.arg("--receipt").arg(&receipt_path);
        }
    }
    let mut child = command.spawn().expect("CLI");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let ready = fs::read_dir(&scratch)
            .expect("scratch entries")
            .filter_map(Result::ok)
            .any(|entry| entry.path().join("inputs").is_dir());
        if ready {
            break;
        }
        if child.try_wait().expect("CLI status").is_some() || Instant::now() > deadline {
            let _ = child.kill();
            let output = child.wait_with_output().expect("CLI output");
            panic!(
                "command did not start: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) },
        0
    );
    let output = child.wait_with_output().expect("cancelled CLI");
    assert_eq!(output.status.code(), Some(5));
    assert!(output.stderr.is_empty());
    let document: Value = serde_json::from_slice(&output.stdout).expect("cancelled JSON");
    assert_eq!(document["status"], "cancelled");
    if command_name == "verify" && !delivery_mode {
        let saved: Value = serde_json::from_slice(&fs::read(receipt_path).expect("receipt"))
            .expect("receipt JSON");
        assert_eq!(saved, document);
    }
    assert_eq!(fs::read_dir(scratch).expect("scratch entries").count(), 0);
    assert!(!delivery_path.exists());
}

#[cfg(unix)]
#[test]
fn replay_interrupt_emits_cancelled_result_and_cleans_workspace() {
    interrupt_running_command("replay", false);
}

#[cfg(unix)]
#[test]
fn verify_interrupt_saves_cancelled_receipt_and_cleans_workspace() {
    interrupt_running_command("verify", false);
}

#[cfg(unix)]
#[test]
fn verify_interrupt_does_not_publish_an_incomplete_delivery() {
    interrupt_running_command("verify", true);
}

#[test]
fn verify_expected_files_are_enforced_and_recorded() {
    for (path, expected_status, expected_code) in [
        ("result.txt", "succeeded", 0),
        ("missing.txt", "assertion_failed", 3),
    ] {
        let (_temp, skill, input) = file_package();
        let output = Command::cargo_bin("skilltape")
            .expect("binary")
            .arg("verify")
            .arg(&skill)
            .arg("--input")
            .arg(&input)
            .args(["--expect-file", path, "--json"])
            .assert()
            .code(expected_code)
            .get_output()
            .stdout
            .clone();
        let receipt: Value = serde_json::from_slice(&output).expect("receipt");
        assert_eq!(receipt["status"], expected_status);
        assert_eq!(receipt["assertions"][0]["target"], path);
        assert_eq!(receipt["assertions"][0]["passed"], expected_code == 0);
    }
}

#[test]
fn verify_expected_files_reject_unsafe_paths_before_running() {
    let (_temp, skill, input) = file_package();
    Command::cargo_bin("skilltape")
        .expect("binary")
        .arg("verify")
        .arg(&skill)
        .arg("--input")
        .arg(&input)
        .args(["--expect-file", "../escape.txt", "--json"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("assertion path is unsafe"));
}

#[test]
fn verify_expected_files_are_bounded() {
    let (_temp, skill, input) = file_package();
    let mut command = Command::cargo_bin("skilltape").expect("binary");
    command.arg("verify").arg(&skill).arg("--input").arg(&input);
    for _ in 0..65 {
        command.args(["--expect-file", "result.txt"]);
    }
    command
        .assert()
        .code(2)
        .stderr(predicates::str::contains("64-file"));
}

#[test]
fn verify_expected_hashes_bind_actual_bytes_and_failed_deliveries_stay_unpublished() {
    use sha2::{Digest, Sha256};

    let expected: String = Sha256::digest(b"secret-cli-value")
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    for (digest, status, code) in [
        (expected.to_uppercase(), "succeeded", 0),
        ("0".repeat(64), "assertion_failed", 3),
    ] {
        let (temp, skill, input) = file_package();
        let delivery = temp.path().join("delivery");
        let output = Command::cargo_bin("skilltape")
            .expect("binary")
            .arg("verify")
            .arg(&skill)
            .arg("--input")
            .arg(&input)
            .arg("--delivery-dir")
            .arg(&delivery)
            .args(["--expect-sha256", &format!("result.txt={digest}"), "--json"])
            .assert()
            .code(code)
            .get_output()
            .stdout
            .clone();
        let receipt: Value = serde_json::from_slice(&output).expect("receipt");
        assert_eq!(receipt["status"], status);
        assert_eq!(receipt["assertions"][0]["kind"], "file_hash");
        assert_eq!(receipt["assertions"][0]["passed"], code == 0);
        assert_eq!(delivery.exists(), code == 0);
        assert!(!String::from_utf8_lossy(&output).contains("secret-cli-value"));
    }
}

#[test]
fn verify_expected_hashes_reject_invalid_requirements_before_loading_a_skill() {
    for requirement in [
        "result.txt",
        "result.txt=bad",
        "result.txt=gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg",
        "=0000000000000000000000000000000000000000000000000000000000000000",
        "../escape=0000000000000000000000000000000000000000000000000000000000000000",
        "./result.txt=0000000000000000000000000000000000000000000000000000000000000000",
    ] {
        Command::cargo_bin("skilltape")
            .expect("binary")
            .args(["verify", "missing-skill", "--expect-sha256", requirement])
            .assert()
            .code(2)
            .stderr(predicates::str::contains("invalid output requirement"));
    }
}

#[test]
fn verify_conflicting_output_hashes_fail_before_loading_a_skill() {
    Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["verify", "missing-skill", "--expect-sha256"])
        .arg(format!("result.txt={}", "0".repeat(64)))
        .arg("--expect-sha256")
        .arg(format!("outputs/result.txt={}", "1".repeat(64)))
        .assert()
        .code(2)
        .stderr(predicates::str::contains("invalid output requirement"));
}

#[test]
fn verify_expected_hashes_share_the_requirement_budget() {
    let mut command = Command::cargo_bin("skilltape").expect("binary");
    command.args(["verify", "missing-skill", "--expect-file", "result.txt"]);
    for _ in 0..64 {
        command.args(["--expect-sha256", &format!("result.txt={}", "0".repeat(64))]);
    }
    command
        .assert()
        .code(2)
        .stderr(predicates::str::contains("64-file"));
}

#[test]
fn optional_context_binds_receipt_without_raw_inputs_or_overwrite() {
    use sha2::{Digest, Sha256};
    let (temp, skill, input) = file_package();
    let receipt = temp.path().join("receipt.json");
    let context = temp.path().join("receipt.json.context");
    Command::cargo_bin("skilltape")
        .expect("binary")
        .arg("verify")
        .arg(&skill)
        .arg("--input")
        .arg(&input)
        .arg("--receipt")
        .arg(&receipt)
        .arg("--context")
        .arg(&context)
        .arg("--expect-file")
        .arg("result.txt")
        .assert()
        .success();
    let original = fs::read(&receipt).unwrap();
    let metadata: skilltape_verify::RunContext =
        serde_json::from_slice(&fs::read(&context).unwrap()).unwrap();
    assert!(metadata.binds_receipt(&original));
    assert_eq!(
        metadata.input_set_sha256,
        skilltape_runner::digest_input_tree(&input).unwrap()
    );
    assert!(!fs::read_to_string(&context)
        .unwrap()
        .contains("secret-cli-value"));
    let digest = Sha256::digest(&original);
    Command::cargo_bin("skilltape")
        .unwrap()
        .args(["verify", "missing-package", "--context"])
        .arg(&context)
        .assert()
        .code(2)
        .stderr(predicates::str::contains("already exists"));
    assert_eq!(Sha256::digest(fs::read(&receipt).unwrap()), digest);
    let mut tampered = metadata;
    tampered.receipt_sha256 = "0".repeat(64);
    assert!(!tampered.binds_receipt(&original));
}
