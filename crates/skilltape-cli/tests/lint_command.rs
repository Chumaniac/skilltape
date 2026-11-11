use std::fs;

use assert_cmd::Command;
use serde_json::Value;

fn initialized_skill() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp = tempfile::tempdir().expect("tempdir");
    let skill = temp.path().join("minimal-skill");

    Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["init", "minimal-skill", "--output"])
        .arg(&skill)
        .assert()
        .success();

    (temp, skill)
}

#[test]
fn lint_accepts_the_checked_in_minimal_skill() {
    let (_temp, skill) = initialized_skill();

    Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["lint"])
        .arg(skill)
        .assert()
        .code(0)
        .stdout(predicates::str::contains("Checked 6 files"))
        .stdout(predicates::str::contains("0 errors"));
}

#[test]
fn lint_prints_stable_policy_code_for_undeclared_executable() {
    let (_temp, skill) = initialized_skill();
    fs::write(
        skill.join("workflow.yaml"),
        "schema: skilltape.dev/workflow/v1\nsteps:\n  - action: exec\n    id: run-python\n    program: python\n    args: []\n    timeout_ms: 1000\n",
    )
    .expect("workflow");

    Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["lint"])
        .arg(skill)
        .assert()
        .code(3)
        .stdout(predicates::str::contains("PKG004"))
        .stdout(predicates::str::contains("workflow.yaml"));
}

#[test]
fn lint_json_output_contains_files_checked_and_errors() {
    let (_temp, skill) = initialized_skill();
    fs::write(
        skill.join("workflow.yaml"),
        "schema: skilltape.dev/workflow/v1\nsteps:\n  - action: exec\n    id: run-python\n    program: python\n    args: []\n    timeout_ms: 1000\n",
    )
    .expect("workflow");

    let output = Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["lint"])
        .arg(skill)
        .arg("--json")
        .assert()
        .code(3)
        .stderr(predicates::str::is_empty())
        .get_output()
        .stdout
        .clone();

    let report: Value = serde_json::from_slice(&output).expect("valid JSON");
    assert_eq!(report["files_checked"], 6);
    let errors = report["errors"].as_array().expect("errors array");
    assert!(errors
        .iter()
        .any(|error| { error["code"] == "PKG004" && error["file"] == "workflow.yaml" }));
}

#[test]
fn lint_maps_schema_diagnostics_to_package_exit_code() {
    let (_temp, skill) = initialized_skill();
    fs::write(
        skill.join("workflow.yaml"),
        "schema: skilltape.dev/workflow/v2\nsteps: []\n",
    )
    .expect("workflow");

    Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["lint"])
        .arg(skill)
        .assert()
        .code(2)
        .stdout(predicates::str::contains("PKG003"))
        .stdout(predicates::str::contains("workflow.yaml"));
}

#[test]
fn lint_strict_preserves_policy_failure_for_lockfile_warnings() {
    let (_temp, skill) = initialized_skill();
    fs::write(
        skill.join("skilltape.lock"),
        "{\"schema\":\"skilltape.dev/lock/v1\",\"engine\":{\"version\":\"0.2.0\"},\"tools\":[],\"scripts\":[]}",
    )
    .expect("lockfile");

    Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["lint"])
        .arg(&skill)
        .assert()
        .success();

    Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["lint", "--strict"])
        .arg(skill)
        .assert()
        .code(3)
        .stdout(predicates::str::contains("PKG010"));
}
