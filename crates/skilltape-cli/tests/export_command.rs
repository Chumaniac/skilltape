use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use serde_json::{json, Value};
use skilltape_core::create_skill_template;
use tempfile::TempDir;

fn package_with_targets(root: &Path, targets: &[&str]) -> PathBuf {
    let package = root.join("skill");
    create_skill_template(&package, "cli-export").expect("skill template");
    let mut manifest: Value =
        serde_yaml::from_slice(&fs::read(package.join("skilltape.yaml")).expect("manifest"))
            .expect("template manifest YAML");
    manifest["targets"] = json!(targets);
    fs::write(
        package.join("skilltape.yaml"),
        serde_yaml::to_string(&manifest).expect("manifest YAML"),
    )
    .expect("manifest rewrite");
    package
}

fn export_command(
    package: &Path,
    target: &str,
    output: &Path,
    json_output: bool,
) -> assert_cmd::assert::Assert {
    let mut command = Command::cargo_bin("skilltape").expect("binary");
    command
        .args(["export"])
        .arg(package)
        .args(["--target", target, "--output"])
        .arg(output);
    if json_output {
        command.arg("--json");
    }
    command.assert()
}

#[test]
fn generic_export_json_is_a_stable_manifest_and_publishes_files() {
    let temp = TempDir::new().expect("temp directory");
    let package = package_with_targets(temp.path(), &["generic-agent-skill"]);
    let output = temp.path().join("generic-output");

    let stdout = export_command(&package, "generic", &output, true)
        .success()
        .stderr(predicates::str::is_empty())
        .get_output()
        .stdout
        .clone();
    let manifest: Value = serde_json::from_slice(&stdout).expect("manifest JSON");

    assert_eq!(manifest["target"], "generic-agent-skill");
    assert!(manifest["package_hash"].as_str().is_some_and(|hash| {
        hash.len() == 64 && hash.chars().all(|character| character.is_ascii_hexdigit())
    }));
    assert!(manifest["files"]
        .as_array()
        .expect("files")
        .iter()
        .any(|file| file == "SKILL.md"));
    assert!(output.join("SKILL.md").is_file());
}

#[test]
fn claude_code_export_uses_the_platform_layout() {
    let temp = TempDir::new().expect("temp directory");
    let package = package_with_targets(temp.path(), &["claude-code"]);
    let output = temp.path().join("claude-output");

    export_command(&package, "claude-code", &output, false)
        .success()
        .stderr(predicates::str::is_empty())
        .stdout(predicates::str::contains("Exported claude-code"));

    assert!(output.join(".claude/skills/cli-export/SKILL.md").is_file());
    assert!(!output.join("SKILL.md").exists());
}

#[test]
fn unknown_target_is_a_policy_error_without_output() {
    let temp = TempDir::new().expect("temp directory");
    let package = package_with_targets(temp.path(), &["generic-agent-skill"]);
    let output = temp.path().join("unknown-output");

    export_command(&package, "future-target", &output, true)
        .code(3)
        .stdout(predicates::str::is_empty())
        .stderr(
            predicates::str::contains("unknown export target")
                .and(predicates::str::contains("generic-agent-skill")),
        );

    assert!(!output.exists());
}

#[test]
fn undeclared_platform_target_is_a_policy_error_without_output() {
    let temp = TempDir::new().expect("temp directory");
    let package = package_with_targets(temp.path(), &["generic-agent-skill"]);
    let output = temp.path().join("undeclared-output");

    export_command(&package, "claude-code", &output, false)
        .code(3)
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::contains("not declared"));

    assert!(!output.exists());
}

#[test]
fn invalid_package_is_an_input_error_without_output() {
    let temp = TempDir::new().expect("temp directory");
    let package = package_with_targets(temp.path(), &["generic-agent-skill"]);
    fs::remove_file(package.join("SKILL.md")).expect("remove required file");
    let output = temp.path().join("invalid-output");

    export_command(&package, "generic-agent-skill", &output, true)
        .code(2)
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::contains("skill package failed to load"));

    assert!(!output.exists());
}

#[test]
fn existing_output_is_not_overwritten() {
    let temp = TempDir::new().expect("temp directory");
    let package = package_with_targets(temp.path(), &["generic-agent-skill"]);
    let output = temp.path().join("existing-output");
    fs::create_dir(&output).expect("output");
    fs::write(output.join("keep.txt"), b"keep this").expect("marker");

    export_command(&package, "generic-agent-skill", &output, false)
        .code(3)
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::contains("already exists"));

    assert_eq!(
        fs::read(output.join("keep.txt")).expect("marker"),
        b"keep this"
    );
    assert!(!output.join("SKILL.md").exists());
}

#[test]
fn repeated_json_exports_have_the_same_manifest() {
    let temp = TempDir::new().expect("temp directory");
    let package = package_with_targets(temp.path(), &["generic-agent-skill"]);
    let first = temp.path().join("first");
    let second = temp.path().join("second");

    let first_stdout = export_command(&package, "generic-agent-skill", &first, true)
        .success()
        .get_output()
        .stdout
        .clone();
    let second_stdout = export_command(&package, "generic-agent-skill", &second, true)
        .success()
        .get_output()
        .stdout
        .clone();

    let first_manifest: Value = serde_json::from_slice(&first_stdout).expect("first JSON");
    let second_manifest: Value = serde_json::from_slice(&second_stdout).expect("second JSON");
    assert_eq!(first_manifest, second_manifest);
}
