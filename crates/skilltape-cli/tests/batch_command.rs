use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use skilltape_core::create_skill_template;
use tempfile::TempDir;

fn sha(value: &[u8]) -> String {
    Sha256::digest(value)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn fixture() -> (TempDir, PathBuf, PathBuf) {
    let temp = TempDir::new().unwrap();
    let materials = temp.path().join("materials");
    let skill = materials.join("skill");
    create_skill_template(&skill, "batch-example").unwrap();
    fs::write(skill.join("workflow.yaml"), serde_json::to_vec(&json!({
        "schema":"skilltape.dev/workflow/v1","steps":[{"action":"file","id":"copy","operation":"copy","from":"inputs/source.txt","to":"outputs/result.txt"}]
    })).unwrap()).unwrap();
    fs::write(skill.join("permissions.json"), serde_json::to_vec(&json!({
        "schema":"skilltape.dev/permissions/v1","filesystem":{"read":["inputs/**","outputs/**"],"write":["outputs/**"]},
        "process":{"executables":[],"max_processes":1,"default_timeout_ms":1000},"network":{"enabled":false,"allow_hosts":[]},"secrets":{"read_environment":false}
    })).unwrap()).unwrap();
    for (id, data) in [
        ("accepted", "PRIVATE_SYNTHETIC_ACCEPTED"),
        ("repair", "PRIVATE_SYNTHETIC_WRONG"),
    ] {
        fs::create_dir_all(materials.join("inputs").join(id)).unwrap();
        fs::write(materials.join("inputs").join(id).join("source.txt"), data).unwrap();
    }
    let plan = materials.join("batch.json");
    fs::write(&plan, serde_json::to_vec(&json!({"schema":"skilltape.dev/batch-plan/v1","skill":"skill","cases":[
        {"id":"accepted","input":"inputs/accepted","expected_files":["outputs/result.txt"],"expected_hashes":[format!("outputs/result.txt={}",sha(b"PRIVATE_SYNTHETIC_ACCEPTED"))]},
        {"id":"repair","input":"inputs/repair","expected_files":["outputs/result.txt"],"expected_hashes":[format!("outputs/result.txt={}",sha(b"PRIVATE_SYNTHETIC_ACCEPTED"))]}
    ]})).unwrap()).unwrap();
    let output = temp.path().join("batch-workspace");
    (temp, plan, output)
}

fn command() -> Command {
    Command::cargo_bin("skilltape").unwrap()
}
fn inspect(root: &Path) -> Value {
    let output = command()
        .args(["batch", "inspect"])
        .arg(root)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(!String::from_utf8_lossy(&output).contains("PRIVATE_SYNTHETIC"));
    serde_json::from_slice(&output).unwrap()
}
fn prepare(plan: &Path, root: &Path) {
    command()
        .args(["batch", "prepare"])
        .arg(plan)
        .arg("--output")
        .arg(root)
        .assert()
        .success();
}

#[test]
fn prepare_freezes_materials_without_running_and_requires_explicit_execution() {
    let (_temp, plan, root) = fixture();
    prepare(&plan, &root);
    let report = inspect(&root);
    assert_eq!(report["status"], "pending");
    assert_eq!(report["cases"].as_array().unwrap().len(), 2);
    assert!(!root.join(".skilltape/receipts/accepted-a1.json").exists());
    command().args(["batch", "run"]).arg(&root).assert().code(2);
    assert_eq!(inspect(&root)["status"], "pending");
    command()
        .args(["batch", "prepare"])
        .arg(plan)
        .arg("--output")
        .arg(&root)
        .assert()
        .code(2);
}

#[test]
fn mixed_batch_retry_preserves_successful_attempts_and_original_materials() {
    let (temp, plan, root) = fixture();
    prepare(&plan, &root);
    command()
        .args(["batch", "run"])
        .arg(&root)
        .args(["--yes", "--json"])
        .assert()
        .code(3);
    let first = inspect(&root);
    assert_eq!(first["status"], "partial");
    assert_eq!(first["cases"][0]["status"], "succeeded");
    assert_eq!(first["cases"][1]["status"], "failed");
    let receipt = root.join(".skilltape/receipts/accepted-a1.json");
    let saved = fs::read(&receipt).unwrap();
    command()
        .args(["batch", "run"])
        .arg(&root)
        .arg("--yes")
        .assert()
        .code(3);
    assert_eq!(inspect(&root)["cases"][1]["attempt_count"], 1);
    let corrected = temp.path().join("corrected");
    fs::create_dir(&corrected).unwrap();
    fs::write(corrected.join("source.txt"), "PRIVATE_SYNTHETIC_ACCEPTED").unwrap();
    command()
        .args(["batch", "retry"])
        .arg(&root)
        .args(["--case", "repair", "--input"])
        .arg(&corrected)
        .arg("--yes")
        .assert()
        .success();
    let final_report = inspect(&root);
    assert_eq!(final_report["status"], "complete");
    assert_eq!(final_report["cases"][0]["attempt_count"], 1);
    assert_eq!(final_report["cases"][1]["attempt_count"], 2);
    assert_eq!(fs::read(&receipt).unwrap(), saved);
    assert_eq!(
        fs::read(plan.parent().unwrap().join("inputs/repair/source.txt")).unwrap(),
        b"PRIVATE_SYNTHETIC_WRONG"
    );
    command()
        .args(["batch", "retry"])
        .arg(&root)
        .args(["--case", "accepted", "--yes"])
        .assert()
        .code(2);
}

#[test]
fn changed_frozen_input_and_saved_evidence_cannot_be_reused_as_success() {
    let (_temp, plan, root) = fixture();
    prepare(&plan, &root);
    fs::write(
        root.join("inputs/accepted/source.txt"),
        "changed frozen input",
    )
    .unwrap();
    command()
        .args(["batch", "run"])
        .arg(&root)
        .arg("--yes")
        .assert()
        .code(2);
    assert!(!root.join(".skilltape/receipts/accepted-a1.json").exists());
}

#[test]
fn unsafe_case_paths_are_rejected_before_creating_a_workspace() {
    let (_temp, plan, root) = fixture();
    let mut value: Value = serde_json::from_slice(&fs::read(&plan).unwrap()).unwrap();
    value["cases"][0]["id"] = json!("../escape");
    fs::write(&plan, serde_json::to_vec(&value).unwrap()).unwrap();
    command()
        .args(["batch", "prepare"])
        .arg(plan)
        .arg("--output")
        .arg(&root)
        .assert()
        .code(2);
    assert!(!root.exists());
}

#[test]
fn successful_evidence_tampering_blocks_retry_and_never_overwrites_originals() {
    let (_temp, plan, root) = fixture();
    prepare(&plan, &root);
    command()
        .args(["batch", "run"])
        .arg(&root)
        .arg("--yes")
        .assert()
        .code(3);
    let path = root.join("accepted-a1/artifacts/result.txt");
    fs::write(&path, "synthetic tampering").unwrap();
    command()
        .args(["batch", "retry"])
        .arg(&root)
        .args(["--case", "repair", "--yes"])
        .assert()
        .code(2);
    assert!(!root.join(".skilltape/receipts/repair-a2.json").exists());
    assert_eq!(fs::read(&path).unwrap(), b"synthetic tampering");
}

#[test]
fn console_cannot_promote_a_failed_receipt_by_changing_only_the_index() {
    let (_temp, plan, root) = fixture();
    prepare(&plan, &root);
    command()
        .args(["batch", "run"])
        .arg(&root)
        .arg("--yes")
        .assert()
        .code(3);
    let path = root.join(".skilltape/batch.json");
    let mut state: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    state["cases"][1]["attempts"][0]["status"] = json!("succeeded");
    state["cases"][1]["attempts"][0]["delivery_manifest_sha256"] = json!("a".repeat(64));
    state["cases"][1]["attempts"][0]["artifact_tree_sha256"] = json!("a".repeat(64));
    fs::write(path, serde_json::to_vec(&state).unwrap()).unwrap();
    let model = skilltape_console_api::ConsoleReadModel::new(&root).unwrap();
    let report = model.batch_review().unwrap();
    assert_eq!(report.status, "unconfirmed");
    assert_eq!(
        report.cases[1].status,
        skilltape_core::batch::BatchStatus::Unconfirmed
    );
}

#[test]
fn interrupted_complete_evidence_is_reconciled_without_executing_again() {
    let (_temp, plan, root) = fixture();
    prepare(&plan, &root);
    command()
        .args(["batch", "run"])
        .arg(&root)
        .arg("--yes")
        .assert()
        .code(3);
    let receipt = root.join(".skilltape/receipts/accepted-a1.json");
    let original = fs::read(&receipt).unwrap();
    let path = root.join(".skilltape/batch.json");
    let mut state: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    state["cases"][0]["attempts"][0]["status"] = json!("running");
    for field in [
        "receipt_sha256",
        "context_sha256",
        "delivery_manifest_sha256",
        "artifact_tree_sha256",
    ] {
        state["cases"][0]["attempts"][0][field] = Value::Null;
    }
    fs::write(path, serde_json::to_vec(&state).unwrap()).unwrap();
    command()
        .args(["batch", "reconcile"])
        .arg(&root)
        .assert()
        .success();
    assert_eq!(inspect(&root)["cases"][0]["status"], "succeeded");
    assert_eq!(inspect(&root)["cases"][0]["attempt_count"], 1);
    assert_eq!(fs::read(receipt).unwrap(), original);
}

#[test]
fn unknown_outcomes_block_automatic_execution_and_explicit_retry() {
    let (_temp, plan, root) = fixture();
    prepare(&plan, &root);
    let path = root.join(".skilltape/batch.json");
    let mut state: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    state["cases"][0]["attempts"] = json!([{"number":1,"status":"running","input_sha256":state["cases"][0]["input_sha256"],"generation":0,"receipt_sha256":null,"context_sha256":null,"delivery_manifest_sha256":null,"artifact_tree_sha256":null,"error_code":null}]);
    fs::write(path, serde_json::to_vec(&state).unwrap()).unwrap();
    command()
        .args(["batch", "run"])
        .arg(&root)
        .arg("--yes")
        .assert()
        .code(2);
    command()
        .args(["batch", "retry"])
        .arg(&root)
        .args(["--case", "accepted", "--yes"])
        .assert()
        .code(2);
    assert_eq!(inspect(&root)["status"], "unconfirmed");
    assert!(!root.join(".skilltape/receipts/accepted-a2.json").exists());
    assert!(!root.join(".skilltape/receipts/repair-a1.json").exists());
}

#[test]
fn batch_lock_prevents_a_second_writer_and_changed_receipts_do_not_show_success() {
    let (_temp, plan, root) = fixture();
    prepare(&plan, &root);
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(".skilltape/batch.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    command()
        .args(["batch", "run"])
        .arg(&root)
        .arg("--yes")
        .assert()
        .code(2);
    drop(lock);
    command()
        .args(["batch", "run"])
        .arg(&root)
        .arg("--yes")
        .assert()
        .code(3);
    fs::write(root.join(".skilltape/receipts/accepted-a1.json"), b"{}").unwrap();
    let report = inspect(&root);
    assert_eq!(report["status"], "unconfirmed");
    assert_eq!(report["cases"][0]["evidence"], "changed");
}

#[test]
fn a_prepared_batch_survives_original_source_removal_and_caps_failed_attempts() {
    let (_temp, plan, root) = fixture();
    prepare(&plan, &root);
    fs::remove_dir_all(plan.parent().unwrap()).unwrap();
    command()
        .args(["batch", "run"])
        .arg(&root)
        .arg("--yes")
        .assert()
        .code(3);
    for _ in 0..2 {
        command()
            .args(["batch", "retry"])
            .arg(&root)
            .args(["--case", "repair", "--yes"])
            .assert()
            .code(3);
    }
    let first = fs::read(root.join(".skilltape/receipts/repair-a1.json")).unwrap();
    command()
        .args(["batch", "retry"])
        .arg(&root)
        .args(["--case", "repair", "--yes"])
        .assert()
        .code(2);
    assert_eq!(inspect(&root)["cases"][1]["attempt_count"], 3);
    assert_eq!(
        fs::read(root.join(".skilltape/receipts/repair-a1.json")).unwrap(),
        first
    );
}

#[cfg(unix)]
#[test]
fn batch_material_symlinks_are_rejected_without_touching_the_link_target() {
    let (temp, plan, root) = fixture();
    let outside = temp.path().join("outside.txt");
    fs::write(&outside, "retained synthetic").unwrap();
    let source = plan.parent().unwrap().join("inputs/accepted/source.txt");
    fs::remove_file(&source).unwrap();
    std::os::unix::fs::symlink(&outside, source).unwrap();
    command()
        .args(["batch", "prepare"])
        .arg(&plan)
        .arg("--output")
        .arg(&root)
        .assert()
        .code(2);
    assert!(!root.exists());
    assert_eq!(fs::read(outside).unwrap(), b"retained synthetic");
}

#[cfg(unix)]
#[test]
fn interrupt_retains_cancelled_evidence_and_does_not_start_the_next_case() {
    use assert_cmd::cargo::CommandCargoExt;
    use std::process::{Command as ProcessCommand, Stdio};
    use std::time::{Duration, Instant};
    let (temp, plan, root) = fixture();
    let skill = plan.parent().unwrap().join("skill");
    let mut workflow: Value =
        serde_json::from_slice(&fs::read(skill.join("workflow.yaml")).unwrap()).unwrap();
    workflow["steps"].as_array_mut().unwrap().insert(
        0,
        json!({"action":"exec","id":"wait","program":"/bin/sleep","args":["4"],"timeout_ms":10000}),
    );
    fs::write(
        skill.join("workflow.yaml"),
        serde_json::to_vec(&workflow).unwrap(),
    )
    .unwrap();
    let mut permissions: Value =
        serde_json::from_slice(&fs::read(skill.join("permissions.json")).unwrap()).unwrap();
    permissions["process"]["executables"] = json!(["/bin/sleep"]);
    permissions["process"]["default_timeout_ms"] = json!(10000);
    fs::write(
        skill.join("permissions.json"),
        serde_json::to_vec(&permissions).unwrap(),
    )
    .unwrap();
    prepare(&plan, &root);
    let scratch = temp.path().join("scratch");
    fs::create_dir(&scratch).unwrap();
    let mut child = ProcessCommand::cargo_bin("skilltape")
        .unwrap()
        .args(["batch", "run"])
        .arg(&root)
        .args(["--yes", "--json"])
        .env("TMPDIR", &scratch)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if fs::read_dir(&scratch)
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry.path().join("inputs").is_dir())
        {
            break;
        }
        if child.try_wait().unwrap().is_some() || Instant::now() > deadline {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!(
                "synthetic runner did not start: {}",
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
    let output = child.wait_with_output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(5),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["cases"][0]["status"], "cancelled");
    assert_eq!(report["cases"][1]["status"], "pending");
    assert!(root.join(".skilltape/receipts/accepted-a1.json").is_file());
    assert!(!root.join("accepted-a1").exists());
    assert!(!root.join(".skilltape/receipts/repair-a1.json").exists());
    assert_eq!(fs::read_dir(scratch).unwrap().count(), 0);
}
