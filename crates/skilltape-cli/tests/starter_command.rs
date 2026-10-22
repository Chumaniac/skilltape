use assert_cmd::Command;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn cli() -> Command {
    Command::cargo_bin("skilltape").unwrap()
}
fn new(scenario: &str, path: &Path) {
    cli()
        .args(["starter", "new", scenario, "--output"])
        .arg(path)
        .assert()
        .success();
}

#[test]
fn scaffold_is_materials_only_and_never_overwrites() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("task");
    new("inventory", &root);
    assert!(root.join("inputs/request.json").is_file());
    assert!(!root.join("outputs").exists());
    cli()
        .args(["starter", "new", "inventory", "--output"])
        .arg(&root)
        .assert()
        .failure();
    let output = temp.path().join("run");
    cli()
        .args(["starter", "run"])
        .arg(&root)
        .arg("--output")
        .arg(&output)
        .assert()
        .failure()
        .stderr(predicates::str::contains("--yes"));
    assert!(!output.exists());
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn producer_checks_inputs_and_reports_reconciliation_without_claiming_balance() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("task");
    new("reconcile", &root);
    cli()
        .current_dir(&root)
        .args([
            "starter",
            "produce",
            "reconcile",
            "--input",
            "inputs/request.json",
        ])
        .assert()
        .success();
    let report: Value =
        serde_json::from_slice(&fs::read(root.join("outputs/report.json")).unwrap()).unwrap();
    assert_eq!(report["balanced"], false);
    assert_eq!(report["rows"][0]["difference"], -1);
    cli()
        .current_dir(&root)
        .args([
            "starter",
            "produce",
            "reconcile",
            "--input",
            "inputs/request.json",
        ])
        .assert()
        .failure();
}
#[test]
fn missing_unsafe_oversized_and_tampered_inventory_is_rejected_before_run() {
    for mode in ["missing", "tampered", "traversal", "oversized", "unknown"] {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("task");
        new("inventory", &root);
        let file = root.join("inputs/files/sample.txt");
        match mode {
            "missing" => fs::remove_file(&file).unwrap(),
            "tampered" => fs::write(&file, "changed").unwrap(),
            "traversal" => fs::write(
                root.join("inputs/request.json"),
                serde_json::to_vec(
                    &json!({"files":[{"path":"../secret","sha256":"a".repeat(64)}]}),
                )
                .unwrap(),
            )
            .unwrap(),
            "oversized" => fs::write(&file, vec![b'x'; 1024 * 1024 + 1]).unwrap(),
            _ => fs::write(
                root.join("inputs/request.json"),
                b"{\"token\":\"synthetic-secret-not-for-errors\"}",
            )
            .unwrap(),
        }
        let output = temp.path().join("run");
        let result = cli()
            .args(["starter", "run"])
            .arg(&root)
            .arg("--output")
            .arg(&output)
            .arg("--yes")
            .assert()
            .failure();
        assert!(!String::from_utf8_lossy(&result.get_output().stderr).contains("synthetic-secret"));
        assert!(!output.exists(), "no published run for invalid inputs");
    }
}
#[cfg(unix)]
#[test]
fn input_and_output_links_are_refused_without_touching_target() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("task");
    new("inventory", &root);
    let outside = temp.path().join("outside");
    fs::write(&outside, b"untouched").unwrap();
    fs::remove_file(root.join("inputs/files/sample.txt")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("inputs/files/sample.txt")).unwrap();
    cli()
        .args(["starter", "run"])
        .arg(&root)
        .arg("--output")
        .arg(temp.path().join("run"))
        .arg("--yes")
        .assert()
        .failure();
    assert_eq!(fs::read(&outside).unwrap(), b"untouched");
    std::os::unix::fs::symlink(temp.path(), temp.path().join("link")).unwrap();
    cli()
        .args(["starter", "new", "inventory", "--output"])
        .arg(temp.path().join("link/escape"))
        .assert()
        .failure();
    assert!(!temp.path().join("escape").exists());
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn both_starters_capture_compile_verify_and_export_actual_artifacts() {
    for scenario in ["inventory", "reconcile"] {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("task");
        new(scenario, &root);
        let output = temp.path().join("run");
        cli()
            .args(["starter", "run"])
            .arg(&root)
            .arg("--output")
            .arg(&output)
            .arg("--yes")
            .assert()
            .success();
        let actual = fs::read(output.join("capture/outputs/report.json")).unwrap();
        assert_eq!(
            actual,
            fs::read(output.join("deliveries/task/artifacts/report.json")).unwrap()
        );
        let receipt: Value =
            serde_json::from_slice(&fs::read(output.join("deliveries/task/receipt.json")).unwrap())
                .unwrap();
        assert_eq!(receipt["status"], "succeeded");
        assert!(output.join("export").is_dir());
        let again = temp.path().join("bad-delivery");
        cli()
            .arg("verify")
            .arg(output.join("skills/task"))
            .arg("--input")
            .arg(output.join("capture/inputs"))
            .arg("--delivery-dir")
            .arg(&again)
            .arg("--expect-sha256")
            .arg(format!("report.json={}", "0".repeat(64)))
            .assert()
            .failure();
        assert!(!again.exists());
        // Preserve the successful delivery even when another run fails.
        assert_eq!(
            actual,
            fs::read(output.join("deliveries/task/artifacts/report.json")).unwrap()
        );
    }
}
