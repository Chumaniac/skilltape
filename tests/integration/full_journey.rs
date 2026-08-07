use std::fs;
use std::process::{Command as ProcessCommand, Stdio};

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn capture_compile_lint_verify_receipt_and_export_form_one_offline_journey() {
    if !replay_sandbox_available() {
        if std::env::var_os("CI").is_some() {
            panic!("platform replay sandbox is unavailable in CI");
        }
        eprintln!("skipping full journey: platform replay sandbox is unavailable");
        return;
    }

    let temp = TempDir::new().expect("temporary directory");
    let workspace = temp.path().join("workspace");
    let tape = temp.path().join("tapes/demo");
    let package = temp.path().join("compiled-skill");
    let receipt = temp.path().join("receipts/demo.json");
    let exported = temp.path().join("exported");
    fs::create_dir(&workspace).expect("workspace");

    let capture = Command::cargo_bin("skilltape")
        .expect("skilltape binary")
        .current_dir(&workspace)
        .args(["capture", "demo", "--command", "/bin/echo", "--output"])
        .arg(&tape)
        .args(["--yes", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let capture_summary: Value = serde_json::from_slice(&capture.stdout).expect("capture JSON");
    assert_eq!(capture_summary["ok"], true);
    assert!(capture_summary["event_count"]
        .as_u64()
        .is_some_and(|count| count >= 4));
    assert!(tape.join("manifest.json").is_file());
    assert!(tape.join("events.jsonl").is_file());

    let compile = Command::cargo_bin("skilltape")
        .expect("skilltape binary")
        .current_dir(&workspace)
        .args(["compile"])
        .arg(&tape)
        .args(["--output"])
        .arg(&package)
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(String::from_utf8_lossy(&compile.stdout).contains("Compiled skill at"));
    assert!(package.join("skilltape.yaml").is_file());

    let lint = Command::cargo_bin("skilltape")
        .expect("skilltape binary")
        .current_dir(&workspace)
        .args(["lint"])
        .arg(&package)
        .args(["--strict", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let lint_report: Value = serde_json::from_slice(&lint.stdout).expect("lint JSON");
    assert_eq!(lint_report["errors"].as_array().map(Vec::len), Some(0));

    let verify = Command::cargo_bin("skilltape")
        .expect("skilltape binary")
        .current_dir(&workspace)
        .args(["verify"])
        .arg(&package)
        .args(["--receipt"])
        .arg(&receipt)
        .args(["--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let receipt_json: Value = serde_json::from_slice(&verify.stdout).expect("receipt JSON");
    assert_eq!(receipt_json["schema"], "skilltape.dev/receipt/v1");
    assert_eq!(receipt_json["status"], "succeeded");
    let written_receipt: Value = serde_json::from_slice(&fs::read(&receipt).expect("receipt file"))
        .expect("written receipt JSON");
    assert_eq!(receipt_json, written_receipt);

    let export = Command::cargo_bin("skilltape")
        .expect("skilltape binary")
        .current_dir(&workspace)
        .args(["export"])
        .arg(&package)
        .args(["--target", "generic", "--output"])
        .arg(&exported)
        .args(["--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let manifest: Value = serde_json::from_slice(&export.stdout).expect("export manifest JSON");
    assert_eq!(manifest["target"], "generic-agent-skill");
    assert!(exported.join("SKILL.md").is_file());
    assert!(exported.join("workflow.yaml").is_file());
}

fn replay_sandbox_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        return ProcessCommand::new("/usr/bin/sandbox-exec")
            .args(["-p", "(version 1) (allow process-exec)", "/bin/true"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
    }

    #[cfg(target_os = "linux")]
    {
        return ProcessCommand::new("bwrap")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
    }

    #[allow(unreachable_code)]
    false
}
