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
