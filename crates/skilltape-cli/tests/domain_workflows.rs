use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

const DOMAINS: [&str; 4] = [
    "code-review",
    "knowledge-reference",
    "data-export",
    "incident-review",
];

#[test]
fn replay_and_verify_report_input_capacity_without_writing_a_receipt() {
    let temp = TempDir::new().expect("capacity fixture");
    let input = temp.path().join("input");
    fs::create_dir(&input).expect("input");
    fs::File::create(input.join("oversized.diff"))
        .expect("sparse fixture")
        .set_len(16 * 1024 * 1024 + 1)
        .expect("fixture size");
    for command in ["replay", "verify"] {
        let receipt = temp.path().join(format!("{command}.json"));
        let mut cli = Command::cargo_bin("skilltape").expect("binary");
        cli.arg(command)
            .arg(example("code-review"))
            .arg("--input")
            .arg(&input)
            .arg("--json");
        if command == "verify" {
            cli.arg("--receipt").arg(&receipt);
        }
        let result = cli.assert().code(2).get_output().clone();
        assert!(String::from_utf8_lossy(&result.stderr).contains("input capacity exceeded"));
        assert!(!receipt.exists(), "capacity failure creates no Receipt");
    }
}

fn fixture_files(domain: &str) -> &'static [&'static str] {
    match domain {
        "code-review" => &["change.diff"],
        "knowledge-reference" => &["note.md", "source.md"],
        "data-export" => &["metrics.csv"],
        "incident-review" => &["incident.md", "runbook.md"],
        _ => panic!("unknown checked-in domain"),
    }
}

fn example(domain: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/domain-workflows")
        .join(domain)
        .canonicalize()
        .expect("checked-in domain package")
}

#[test]
fn domain_packages_lint_and_export_to_the_registered_targets() {
    for domain in DOMAINS {
        let package = example(domain);
        Command::cargo_bin("skilltape")
            .expect("binary")
            .arg("lint")
            .arg(&package)
            .args(["--strict", "--json"])
            .assert()
            .success();
        let temp = TempDir::new().expect("export directory");
        for target in ["generic", "claude-code", "codex", "cursor"] {
            let output = temp.path().join(target);
            Command::cargo_bin("skilltape")
                .expect("binary")
                .arg("export")
                .arg(&package)
                .args(["--target", target, "--output"])
                .arg(&output)
                .arg("--json")
                .assert()
                .success();
            let entry = match target {
                "claude-code" => format!(".claude/skills/{domain}/SKILL.md"),
                "codex" => format!(".agents/skills/{domain}/SKILL.md"),
                "cursor" => format!(".cursor/skills/{domain}/SKILL.md"),
                _ => "SKILL.md".to_owned(),
            };
            assert!(output.join(entry).is_file());
        }
    }
}

#[test]
fn domain_receipts_reject_changed_or_missing_material() {
    for domain in DOMAINS {
        let package = example(domain);
        let source = package.join("fixtures/input");
        let temp = TempDir::new().expect("input directory");
        let input = temp.path().join("input");
        fs::create_dir(&input).expect("synthetic input");
        for filename in fixture_files(domain) {
            fs::copy(source.join(filename), input.join(filename)).expect("copy fixture");
        }
        let output = Command::cargo_bin("skilltape")
            .expect("binary")
            .arg("verify")
            .arg(&package)
            .arg("--input")
            .arg(&input)
            .arg("--receipt")
            .arg(temp.path().join("passed.json"))
            .arg("--json")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let receipt: Value = serde_json::from_slice(&output).expect("receipt JSON");
        assert_eq!(receipt["status"], "succeeded");
        let first = input.join(fixture_files(domain)[0]);
        fs::write(first, "synthetic changed material").expect("change test input");
        let failed = Command::cargo_bin("skilltape")
            .expect("binary")
            .arg("verify")
            .arg(&package)
            .arg("--input")
            .arg(&input)
            .arg("--receipt")
            .arg(temp.path().join("failed.json"))
            .arg("--json")
            .assert()
            .code(3)
            .get_output()
            .stdout
            .clone();
        let receipt: Value = serde_json::from_slice(&failed).expect("failed receipt JSON");
        assert_eq!(receipt["status"], "run_failed");
        assert!(!String::from_utf8_lossy(&failed).contains("synthetic changed material"));
        fs::remove_file(input.join(fixture_files(domain)[0])).expect("remove required material");
        let missing = Command::cargo_bin("skilltape")
            .expect("binary")
            .arg("verify")
            .arg(&package)
            .arg("--input")
            .arg(&input)
            .arg("--receipt")
            .arg(temp.path().join("missing.json"))
            .arg("--json")
            .assert()
            .code(3)
            .get_output()
            .stdout
            .clone();
        let receipt: Value = serde_json::from_slice(&missing).expect("missing-input receipt JSON");
        assert_eq!(receipt["status"], "run_failed");
    }
}

#[test]
fn incident_review_denies_a_copy_to_an_undeclared_output() {
    let source = example("incident-review");
    let temp = TempDir::new().expect("package directory");
    let package = temp.path().join("package");
    fs::create_dir(&package).expect("synthetic package");
    for filename in [
        "README.md",
        "SKILL.md",
        "skilltape.yaml",
        "workflow.yaml",
        "permissions.json",
        "skilltape.lock",
    ] {
        fs::copy(source.join(filename), package.join(filename)).expect("copy package file");
    }
    let workflow_path = package.join("workflow.yaml");
    let mut workflow: Value = serde_json::from_slice(&fs::read(&workflow_path).expect("workflow"))
        .expect("workflow JSON");
    workflow["steps"][0]["to"] = "outputs/unapproved/incident.md".into();
    fs::write(
        &workflow_path,
        serde_json::to_vec(&workflow).expect("modified workflow"),
    )
    .expect("write synthetic workflow");
    let output = Command::cargo_bin("skilltape")
        .expect("binary")
        .arg("verify")
        .arg(&package)
        .arg("--input")
        .arg(source.join("fixtures/input"))
        .arg("--receipt")
        .arg(temp.path().join("denied.json"))
        .arg("--json")
        .assert()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let receipt: Value = serde_json::from_slice(&output).expect("denied receipt JSON");
    assert_eq!(receipt["status"], "run_failed");
    assert_eq!(receipt["steps"].as_array().expect("steps").len(), 1);
    assert_eq!(receipt["steps"][0]["status"], "denied");
    assert!(receipt["policy_decisions"]
        .as_array()
        .expect("policy decisions")
        .iter()
        .any(|decision| decision["allowed"] == false
            && decision["code"] == skilltape_policy::codes::WRITE_SCOPE));
    assert!(!String::from_utf8_lossy(&output).contains("example-queue-delay"));
}
