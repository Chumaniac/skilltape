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
