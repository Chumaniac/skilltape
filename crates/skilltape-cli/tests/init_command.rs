#[test]
fn init_creates_a_lintable_skill_package() {
    let temp = tempfile::tempdir().expect("tempdir");
    let output = temp.path().join("minimal-skill");

    assert_cmd::Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["init", "minimal-skill", "--output"])
        .arg(&output)
        .assert()
        .success();

    assert!(output.join("skilltape.yaml").exists());
    assert!(output.join("workflow.yaml").exists());
    assert!(output.join("permissions.json").exists());
    assert!(output.join("skilltape.lock").exists());
}

#[test]
fn init_refuses_to_overwrite_without_force() {
    let temp = tempfile::tempdir().expect("tempdir");
    let output = temp.path().join("existing");
    std::fs::create_dir_all(&output).expect("directory");
    std::fs::write(output.join("README.md"), "keep me").expect("file");

    assert_cmd::Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["init", "existing", "--output"])
        .arg(&output)
        .assert()
        .failure();
}

#[test]
fn lint_rejects_a_missing_package() {
    let temp = tempfile::tempdir().expect("tempdir");
    let missing = temp.path().join("missing-skill");

    assert_cmd::Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["lint"])
        .arg(missing)
        .assert()
        .code(2)
        .stderr(predicates::str::contains("invalid package root"));
}

#[test]
fn lint_accepts_a_generated_package() {
    let temp = tempfile::tempdir().expect("tempdir");
    let output = temp.path().join("minimal-skill");

    assert_cmd::Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["init", "minimal-skill", "--output"])
        .arg(&output)
        .assert()
        .success();

    assert_cmd::Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["lint"])
        .arg(&output)
        .assert()
        .success()
        .stdout(predicates::str::contains("Checked 6 files: 0 errors, 0 warnings"));
}

#[test]
fn lint_json_reports_a_load_failure() {
    let temp = tempfile::tempdir().expect("tempdir");
    let missing = temp.path().join("missing-skill");

    let output = assert_cmd::Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["lint"])
        .arg(missing)
        .arg("--json")
        .assert()
        .code(2)
        .stderr(predicates::str::contains("invalid package root"))
        .stdout(predicates::str::is_empty())
        .get_output()
        .stderr
        .clone();

    assert!(!output.is_empty());
}

#[test]
fn lint_json_reports_a_clean_generated_package() {
    let temp = tempfile::tempdir().expect("tempdir");
    let output = temp.path().join("minimal-skill");

    assert_cmd::Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["init", "minimal-skill", "--output"])
        .arg(&output)
        .assert()
        .success();

    let first = assert_cmd::Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["lint"])
        .arg(&output)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second = assert_cmd::Command::cargo_bin("skilltape")
        .expect("binary")
        .args(["lint"])
        .arg(&output)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(first, second);
    let report: serde_json::Value = serde_json::from_slice(&first).expect("valid JSON");
    assert_eq!(report["files_checked"], 6);
    assert_eq!(report["errors"].as_array().map(Vec::len), Some(0));
    assert_eq!(report["warnings"].as_array().map(Vec::len), Some(0));
}
