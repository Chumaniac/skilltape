use std::fs;
use std::path::PathBuf;

use serde_json::json;
use skilltape_core::{create_skill_template, SkillPackage};
use skilltape_export::{ClaudeCodeExporter, ExportError, Exporter, GenericExporter};
use tempfile::TempDir;

fn package(targets: &[&str], name: &str) -> (TempDir, PathBuf) {
    let temp = TempDir::new().expect("temp directory");
    let root = temp.path().join("package");
    create_skill_template(&root, "claude-test").expect("template");
    let targets = targets
        .iter()
        .map(|target| (*target).to_owned())
        .collect::<Vec<_>>();
    fs::write(
        root.join("skilltape.yaml"),
        serde_json::to_vec(&json!({
            "schema": "skilltape.dev/skill/v1",
            "name": name,
            "version": "0.1.0",
            "description": "Claude Code export test.",
            "engine": {"min_version": "0.1.0"},
            "entrypoint": {
                "workflow": "workflow.yaml",
                "permissions": "permissions.json",
                "lockfile": "skilltape.lock"
            },
            "inputs": [],
            "outputs": [],
            "targets": targets
        }))
        .expect("manifest JSON"),
    )
    .expect("manifest");
    (temp, root)
}

#[test]
fn claude_export_uses_the_expected_layout_and_preserves_core_bytes() {
    let (temp, root) = package(&["generic-agent-skill", "claude-code"], "claude-test");
    let output = temp.path().join("exported");
    let loaded = SkillPackage::load(&root).expect("package");
    let generic_output = temp.path().join("generic");
    let generic = GenericExporter
        .export(&loaded, &generic_output)
        .expect("generic export");

    let manifest = ClaudeCodeExporter
        .export(&loaded, &output)
        .expect("Claude export");
    let package_root = output.join(".claude/skills/claude-test");
    assert_eq!(manifest.target, "claude-code");
    assert_eq!(manifest.package_hash, generic.package_hash);
    assert!(manifest
        .files
        .iter()
        .all(|file| file.starts_with(".claude/skills/claude-test/")));
    assert_eq!(
        fs::read(package_root.join("workflow.yaml")).expect("exported workflow"),
        fs::read(root.join("workflow.yaml")).expect("source workflow")
    );
    assert_eq!(
        fs::read(package_root.join("permissions.json")).expect("exported permissions"),
        fs::read(root.join("permissions.json")).expect("source permissions")
    );
    assert!(!output.join("workflow.yaml").exists());
}

#[test]
fn repeated_claude_exports_are_deterministic() {
    let (temp, root) = package(&["claude-code"], "claude-test");
    let loaded = SkillPackage::load(&root).expect("package");
    let first = ClaudeCodeExporter
        .export(&loaded, &temp.path().join("first"))
        .expect("first export");
    let second = ClaudeCodeExporter
        .export(&loaded, &temp.path().join("second"))
        .expect("second export");
    assert_eq!(first, second);
}

#[test]
fn missing_target_and_unsafe_name_are_rejected_before_output() {
    let (temp, root) = package(&["generic-agent-skill"], "claude-test");
    let loaded = SkillPackage::load(&root).expect("package");
    let output = temp.path().join("missing-target");
    let error = ClaudeCodeExporter
        .export(&loaded, &output)
        .expect_err("missing target");
    assert!(matches!(error, ExportError::TargetNotDeclared { .. }));
    assert!(!output.exists());

    let (temp, root) = package(&["claude-code"], "bad/name");
    let loaded = SkillPackage::load(&root).expect("package");
    let output = temp.path().join("unsafe-name");
    let error = ClaudeCodeExporter
        .export(&loaded, &output)
        .expect_err("unsafe name");
    assert!(matches!(error, ExportError::InvalidTargetName { .. }));
    assert!(!output.exists());
}

#[test]
fn existing_output_is_not_overwritten() {
    let (temp, root) = package(&["claude-code"], "claude-test");
    let loaded = SkillPackage::load(&root).expect("package");
    let output = temp.path().join("existing");
    fs::create_dir(&output).expect("output");
    fs::write(output.join("keep.txt"), "keep").expect("marker");

    let error = ClaudeCodeExporter
        .export(&loaded, &output)
        .expect_err("existing output");
    assert!(matches!(error, ExportError::OutputExists { .. }));
    assert_eq!(
        fs::read_to_string(output.join("keep.txt")).expect("marker"),
        "keep"
    );
}

#[cfg(unix)]
#[test]
fn symlinked_output_parent_is_rejected() {
    let (temp, root) = package(&["claude-code"], "claude-test");
    let loaded = SkillPackage::load(&root).expect("package");
    let outside = temp.path().join("outside");
    let linked = temp.path().join("linked");
    fs::create_dir(&outside).expect("outside");
    std::os::unix::fs::symlink(&outside, &linked).expect("symlink");

    let error = ClaudeCodeExporter
        .export(&loaded, &linked.join("exported"))
        .expect_err("symlink parent");
    assert!(matches!(error, ExportError::UnsafeOutput { .. }));
    assert!(!outside.join("exported").exists());
}
