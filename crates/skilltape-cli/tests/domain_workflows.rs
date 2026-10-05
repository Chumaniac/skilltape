use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

const DOMAINS: [&str; 3] = ["code-review", "knowledge-reference", "data-export"];

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
fn domain_receipts_verify_fixture_integrity_and_reject_changed_material() {
    for domain in DOMAINS {
        let package = example(domain);
        let source = package.join("fixtures/input");
        let temp = TempDir::new().expect("input directory");
        let input = temp.path().join("input");
        fs::create_dir(&input).expect("synthetic input");
        for entry in fs::read_dir(&source).expect("fixtures") {
            let entry = entry.expect("fixture entry");
            fs::copy(entry.path(), input.join(entry.file_name())).expect("copy fixture");
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
        let first = fs::read_dir(&input)
            .expect("input files")
            .next()
            .expect("one input")
            .expect("entry")
            .path();
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
    }
}
