#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use skilltape_core::create_skill_template;
use tempfile::TempDir;

fn fixture() -> (TempDir, PathBuf, PathBuf) {
    let temp = TempDir::new().expect("fixture");
    let package = temp.path().join("package");
    let input = temp.path().join("input");
    create_skill_template(&package, "delivery-example").expect("package");
    fs::create_dir(&input).expect("input");
    fs::write(input.join("source.txt"), "private synthetic material").expect("input file");
    fs::write(
        package.join("workflow.yaml"),
        serde_json::to_vec(&json!({
            "schema":"skilltape.dev/workflow/v1", "steps":[{
                "action":"file","id":"copy","operation":"copy",
                "from":"inputs/source.txt","to":"outputs/result.txt"
            }]
        }))
        .expect("workflow"),
    )
    .expect("write workflow");
    fs::write(package.join("permissions.json"), serde_json::to_vec(&json!({
        "schema":"skilltape.dev/permissions/v1", "filesystem":{"read":["inputs/**","outputs/**"],"write":["outputs/**"]},
        "process":{"executables":[],"max_processes":1,"default_timeout_ms":1000},
        "network":{"enabled":false,"allow_hosts":[]},"secrets":{"read_environment":false}
    })).expect("permissions")).expect("write permissions");
    (temp, package, input)
}

fn verify(package: &Path, input: &Path, target: &Path) -> Command {
    let mut command = Command::cargo_bin("skilltape").expect("binary");
    command
        .arg("verify")
        .arg(package)
        .arg("--input")
        .arg(input)
        .arg("--delivery-dir")
        .arg(target)
        .arg("--json");
    command
}

#[test]
fn delivery_keeps_actual_artifacts_and_binds_the_exact_receipt() {
    let (temp, package, input) = fixture();
    let target = temp.path().join("delivery");
    let stdout = verify(&package, &input, &target)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let receipt_bytes = fs::read(target.join("receipt.json")).expect("persisted Receipt");
    let receipt: Value = serde_json::from_slice(&receipt_bytes).expect("Receipt");
    assert_eq!(
        receipt,
        serde_json::from_slice::<Value>(&stdout).expect("stdout Receipt")
    );
    assert_eq!(receipt["status"], "succeeded");
    assert_eq!(
        fs::read(target.join("artifacts/result.txt")).expect("retained artifact"),
        b"private synthetic material"
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(target.join("delivery.json")).expect("manifest"))
            .expect("manifest JSON");
    assert_eq!(manifest["schema"], "skilltape.dev/delivery/v1");
    assert_eq!(
        manifest["receipt_sha256"],
        hex(Sha256::digest(&receipt_bytes))
    );
    assert_eq!(manifest["run_id"], receipt["run_id"]);
    assert_eq!(manifest["files"][0]["path"], "result.txt");
    assert_eq!(manifest["files"][0]["bytes"], 26);
    assert_eq!(
        manifest["files"][0]["sha256"],
        hex(Sha256::digest(b"private synthetic material"))
    );
    let mut digest = Sha256::new();
    digest.update(b"result.txt\0");
    digest.update(26_u64.to_be_bytes());
    digest.update(hex(Sha256::digest(b"private synthetic material")).as_bytes());
    assert_eq!(manifest["artifact_set_sha256"], hex(digest.finalize()));
    assert!(!String::from_utf8_lossy(&stdout).contains("private synthetic material"));
}

#[test]
fn delivery_rejects_existing_destinations_and_input_overlap() {
    let (temp, package, input) = fixture();
    let target = temp.path().join("delivery");
    fs::create_dir(&target).expect("existing directory");
    fs::write(target.join("owner.txt"), "original").expect("existing material");
    verify(&package, &input, &target).assert().code(2);
    assert_eq!(
        fs::read(target.join("owner.txt")).expect("original"),
        b"original"
    );
    assert!(!target.join("receipt.json").exists());
    verify(&package, &input, &input.join("delivery"))
        .assert()
        .code(2);
    verify(&package, &input, &package.join("delivery"))
        .assert()
        .code(2);
    assert!(!input.join("delivery").exists());
    assert!(!package.join("delivery").exists());
}

#[test]
fn failed_workflow_and_conflicting_receipt_do_not_publish() {
    let (temp, package, input) = fixture();
    let target = temp.path().join("delivery");
    verify(&package, &input, &target)
        .arg("--receipt")
        .arg(temp.path().join("separate.json"))
        .assert()
        .code(2);
    assert!(!target.exists());
    fs::remove_file(input.join("source.txt")).expect("missing required source");
    verify(&package, &input, &target).assert().code(3);
    assert!(!target.exists());
}

#[cfg(unix)]
#[test]
fn delivery_rejects_symlink_parents_and_alias_overlap() {
    use std::os::unix::fs::symlink;
    let (temp, package, input) = fixture();
    let alias = temp.path().join("alias");
    symlink(&input, &alias).expect("alias");
    verify(&package, &input, &alias.join("delivery"))
        .assert()
        .code(2);
    assert!(!input.join("delivery").exists());
    assert!(
        skilltape_runner::validate_output_root(&package, &input, &alias.join("delivery")).is_err()
    );
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
