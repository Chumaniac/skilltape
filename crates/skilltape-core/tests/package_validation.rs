use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use skilltape_core::{DiagnosticLevel, PackageError, SkillPackage};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestPackage {
    root: PathBuf,
}

impl TestPackage {
    fn valid() -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "skilltape-package-validation-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("temporary package root should be created");

        let package = Self { root };
        package.write(
            "skilltape.yaml",
            r#"schema: skilltape.dev/skill/v1
name: test-skill
version: 0.1.0
description: Test package.
engine:
  min_version: 0.1.0
entrypoint:
  workflow: workflow.yaml
  permissions: permissions.json
  lockfile: skilltape.lock
inputs:
  - id: source
    type: file
    required: true
outputs:
  - id: result
    type: file
    path: output/result.txt
targets:
  - generic-agent-skill
"#,
        );
        package.write(
            "workflow.yaml",
            r#"schema: skilltape.dev/workflow/v1
steps:
  - id: copy-input
    action: exec
    program: cp
    args:
      - "{{ inputs.source }}"
      - output/result.txt
    timeout_ms: 1000
    outputs:
      - path: output/result.txt
        type: file
"#,
        );
        package.write(
            "permissions.json",
            r#"{
  "schema": "skilltape.dev/permissions/v1",
  "filesystem": {
    "read": ["inputs/**", "work/**", "scripts/**"],
    "write": ["work/**", "output/**"]
  },
  "process": {
    "executables": ["cp"],
    "max_processes": 1,
    "default_timeout_ms": 1000
  },
  "network": {"enabled": false, "allow_hosts": []},
  "secrets": {"read_environment": false}
}"#,
        );
        package.write(
            "skilltape.lock",
            r#"{
  "schema": "skilltape.dev/lock/v1",
  "engine": {"version": "0.1.0"},
  "tools": [{"program": "cp", "version": "1.0.0"}],
  "scripts": []
}"#,
        );
        package.write("SKILL.md", "# Test Skill\n");
        package.write("README.md", "# Test Package\n");
        package
    }

    fn path(&self, file: &str) -> PathBuf {
        self.root.join(file)
    }

    fn write(&self, file: &str, contents: &str) {
        fs::write(self.path(file), contents).expect("fixture file should be written");
    }
}

impl Drop for TestPackage {
    fn drop(&mut self) {
        let temp = std::env::temp_dir();
        assert!(self.root.starts_with(&temp));
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn has_diagnostic(
    diagnostics: &[skilltape_core::Diagnostic],
    code: &str,
    level: DiagnosticLevel,
    file: &str,
    path: &str,
) -> bool {
    diagnostics.iter().any(|diagnostic| {
        diagnostic.code == code
            && diagnostic.level == level
            && diagnostic.file == file
            && diagnostic.path == path
    })
}

#[test]
fn loads_all_required_package_files() {
    let package = TestPackage::valid();

    let loaded = SkillPackage::load(&package.root).expect("valid package should load");

    assert_eq!(loaded.root, package.root.canonicalize().unwrap());
    assert_eq!(loaded.manifest.name, "test-skill");
    assert_eq!(loaded.workflow.steps.len(), 1);
    assert_eq!(loaded.permissions.process.executables, ["cp"]);
    assert_eq!(loaded.lockfile.tools.len(), 1);
    let report = loaded.lint(false);
    assert_eq!(report.files_checked, 6);
    assert!(report.errors.is_empty(), "unexpected errors: {:?}", report.errors);
    assert!(report.warnings.is_empty());
}

#[test]
fn reports_missing_entrypoint_file() {
    let package = TestPackage::valid();
    fs::remove_file(package.path("workflow.yaml")).unwrap();

    let error = SkillPackage::load(&package.root).expect_err("missing file must fail loading");

    assert!(matches!(
        error,
        PackageError::MissingRequiredFile { ref file } if file == "workflow.yaml"
    ));
    assert!(error.to_string().contains("PKG001"));
}

#[test]
fn reports_workflow_program_without_process_permission() {
    let package = TestPackage::valid();
    package.write(
        "permissions.json",
        &fs::read_to_string(package.path("permissions.json"))
            .unwrap()
            .replace("[\"cp\"]", "[]"),
    );

    let report = SkillPackage::load(&package.root).unwrap().lint(false);

    assert!(has_diagnostic(
        &report.errors,
        "PKG004",
        DiagnosticLevel::Error,
        "workflow.yaml",
        "steps[0].program"
    ));
}

#[test]
fn reports_step_output_outside_declared_write_scope() {
    let package = TestPackage::valid();
    package.write(
        "permissions.json",
        &fs::read_to_string(package.path("permissions.json"))
            .unwrap()
            .replace("[\"work/**\", \"output/**\"]", "[\"work/**\"]"),
    );

    let report = SkillPackage::load(&package.root).unwrap().lint(false);

    assert!(has_diagnostic(
        &report.errors,
        "PKG006",
        DiagnosticLevel::Error,
        "workflow.yaml",
        "steps[0].outputs[0].path"
    ));
}

#[test]
fn strict_mode_turns_environment_mismatch_into_error() {
    let package = TestPackage::valid();
    package.write(
        "skilltape.lock",
        &fs::read_to_string(package.path("skilltape.lock"))
            .unwrap()
            .replace("\"0.1.0\"", "\"0.0.9\""),
    );
    let loaded = SkillPackage::load(&package.root).unwrap();

    let regular = loaded.lint(false);
    let strict = loaded.lint(true);

    assert!(has_diagnostic(
        &regular.warnings,
        "PKG010",
        DiagnosticLevel::Warning,
        "skilltape.lock",
        "engine.version"
    ));
    assert!(has_diagnostic(
        &strict.errors,
        "PKG010",
        DiagnosticLevel::Error,
        "skilltape.lock",
        "engine.version"
    ));
}

#[test]
fn rejects_invalid_json_without_echoing_file_contents() {
    let package = TestPackage::valid();
    package.write("permissions.json", "{ secret-package-content");

    let error = SkillPackage::load(&package.root).expect_err("invalid JSON must fail loading");

    assert!(matches!(
        error,
        PackageError::InvalidFile { ref file, .. } if file == "permissions.json"
    ));
    assert!(!error.to_string().contains("secret-package-content"));
}

#[test]
fn reports_json_schema_mismatch_with_source_path() {
    let package = TestPackage::valid();
    package.write(
        "workflow.yaml",
        &fs::read_to_string(package.path("workflow.yaml"))
            .unwrap()
            .replace("steps:", "unknown_field: true\nsteps:"),
    );

    let report = SkillPackage::load(&package.root).unwrap().lint(false);

    assert!(has_diagnostic(
        &report.errors,
        "PKG003",
        DiagnosticLevel::Error,
        "workflow.yaml",
        "unknown_field"
    ));
}

#[test]
fn reports_entrypoint_path_mismatch() {
    let package = TestPackage::valid();
    package.write(
        "skilltape.yaml",
        &fs::read_to_string(package.path("skilltape.yaml"))
            .unwrap()
            .replace("workflow: workflow.yaml", "workflow: nested/workflow.yaml"),
    );

    let report = SkillPackage::load(&package.root).unwrap().lint(false);

    assert!(has_diagnostic(
        &report.errors,
        "PKG002",
        DiagnosticLevel::Error,
        "skilltape.yaml",
        "entrypoint.workflow"
    ));
}

#[test]
fn reports_undeclared_read_and_unsafe_traversal_paths() {
    let package = TestPackage::valid();
    package.write(
        "workflow.yaml",
        r#"schema: skilltape.dev/workflow/v1
steps:
  - id: run-script
    action: script
    path: private/../scripts/build.py
    args: []
    timeout_ms: 1000
"#,
    );

    let report = SkillPackage::load(&package.root).unwrap().lint(false);

    assert!(has_diagnostic(
        &report.errors,
        "PKG005",
        DiagnosticLevel::Error,
        "workflow.yaml",
        "steps[0].path"
    ));
    assert!(has_diagnostic(
        &report.errors,
        "PKG007",
        DiagnosticLevel::Error,
        "workflow.yaml",
        "steps[0].path"
    ));
}

#[test]
fn reports_undeclared_input_and_manifest_output_mismatch() {
    let package = TestPackage::valid();
    package.write(
        "workflow.yaml",
        &fs::read_to_string(package.path("workflow.yaml"))
            .unwrap()
            .replace("inputs.source", "inputs.missing")
            .replace("output/result.txt", "output/other.txt"),
    );

    let report = SkillPackage::load(&package.root).unwrap().lint(false);

    assert!(has_diagnostic(
        &report.errors,
        "PKG008",
        DiagnosticLevel::Error,
        "workflow.yaml",
        "steps[0].args[0]"
    ));
    assert!(has_diagnostic(
        &report.errors,
        "PKG009",
        DiagnosticLevel::Error,
        "workflow.yaml",
        "steps[0].outputs[0].path"
    ));
}

#[test]
fn reports_lockfile_tool_mismatch() {
    let package = TestPackage::valid();
    package.write(
        "skilltape.lock",
        &fs::read_to_string(package.path("skilltape.lock"))
            .unwrap()
            .replace(
                "[{\"program\": \"cp\", \"version\": \"1.0.0\"}]",
                "[]",
            ),
    );

    let report = SkillPackage::load(&package.root).unwrap().lint(false);

    assert!(has_diagnostic(
        &report.errors,
        "PKG010",
        DiagnosticLevel::Error,
        "skilltape.lock",
        "tools"
    ));
}

#[cfg(unix)]
#[test]
fn rejects_required_file_symlink_that_escapes_package_root() {
    use std::os::unix::fs::symlink;

    let package = TestPackage::valid();
    let outside = std::env::temp_dir().join(format!(
        "skilltape-outside-permissions-{}",
        std::process::id()
    ));
    fs::write(&outside, "{}").unwrap();
    fs::remove_file(package.path("permissions.json")).unwrap();
    symlink(&outside, package.path("permissions.json")).unwrap();

    let error = SkillPackage::load(&package.root).expect_err("escaping symlink must fail");

    assert!(matches!(
        error,
        PackageError::UnsafePackagePath { ref file } if file == "permissions.json"
    ));
    assert!(error.to_string().contains("PKG007"));
    fs::remove_file(outside).unwrap();
}

#[test]
fn rejects_required_path_that_is_not_a_complete_file() {
    let package = TestPackage::valid();
    fs::remove_file(package.path("README.md")).unwrap();
    fs::create_dir(package.path("README.md")).unwrap();

    let error = SkillPackage::load(&package.root).expect_err("directory is not a package file");

    assert!(matches!(
        error,
        PackageError::IncompleteRequiredFile { ref file } if file == "README.md"
    ));
}

fn _assert_path_is_path(_: &Path) {}
