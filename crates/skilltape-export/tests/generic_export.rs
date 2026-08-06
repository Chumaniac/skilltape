use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;
use skilltape_core::{create_skill_template, SkillPackage};
use skilltape_export::{ExportError, Exporter, GenericExporter};
use tempfile::TempDir;

fn minimal_package() -> (TempDir, PathBuf) {
    let temp = TempDir::new().expect("temp directory");
    let package = temp.path().join("package");
    create_skill_template(&package, "export-test").expect("template");
    (temp, package)
}

fn script_package() -> (TempDir, PathBuf) {
    let (temp, package) = minimal_package();
    fs::create_dir(package.join("scripts")).expect("scripts");
    fs::write(
        package.join("scripts/run.sh"),
        "#!/bin/sh\nprintf exported\n",
    )
    .expect("script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(package.join("scripts/run.sh"))
            .expect("script metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(package.join("scripts/run.sh"), permissions).expect("permissions");
    }
    fs::write(
        package.join("workflow.yaml"),
        serde_json::to_vec(&json!({
            "schema": "skilltape.dev/workflow/v1",
            "steps": [{
                "action": "script",
                "id": "run",
                "path": "scripts/run.sh",
                "args": [],
                "timeout_ms": 1000
            }]
        }))
        .expect("workflow JSON"),
    )
    .expect("workflow");
    fs::write(
        package.join("permissions.json"),
        serde_json::to_vec(&json!({
            "schema": "skilltape.dev/permissions/v1",
            "filesystem": {"read": ["scripts/**"], "write": []},
            "process": {
                "executables": ["scripts/run.sh"],
                "max_processes": 1,
                "default_timeout_ms": 1000
            },
            "network": {"enabled": false, "allow_hosts": []},
            "secrets": {"read_environment": false}
        }))
        .expect("permissions JSON"),
    )
    .expect("permissions");
    fs::write(
        package.join("skilltape.lock"),
        serde_json::to_vec(&json!({
            "schema": "skilltape.dev/lock/v1",
            "engine": {"version": "0.1.0"},
            "tools": [],
            "scripts": [{"path": "scripts/run.sh", "sha256": "script"}]
        }))
        .expect("lock JSON"),
    )
    .expect("lock");
    (temp, package)
}

fn exported_files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut entries = fs::read_dir(current)
            .expect("read export")
            .collect::<Result<Vec<_>, _>>()
            .expect("export entries");
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                collect(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root).expect("relative export").to_owned(),
                    fs::read(path).expect("export file"),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    collect(root, root, &mut files);
    files
}

#[test]
fn generic_export_copies_core_optional_and_referenced_files() {
    let (temp, package) = script_package();
    fs::create_dir_all(package.join("fixtures/input")).expect("fixtures");
    fs::create_dir_all(package.join("receipts")).expect("receipts");
    fs::write(package.join("fixtures/input/example.json"), "fixture").expect("fixture");
    fs::write(
        package.join("receipts/run.json"),
        "{\"status\":\"succeeded\"}",
    )
    .expect("receipt");
    fs::write(package.join("compile.json"), "{\"schema\":\"compile\"}").expect("compile");
    fs::write(package.join("LICENSE"), "MIT").expect("license");
    let output = temp.path().join("exported");

    let loaded = SkillPackage::load(&package).expect("package");
    let manifest = GenericExporter.export(&loaded, &output).expect("export");

    assert_eq!(manifest.target, "generic-agent-skill");
    assert!(manifest.files.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(manifest.files.iter().any(|file| file == "scripts/run.sh"));
    assert!(manifest
        .files
        .iter()
        .any(|file| file == "fixtures/input/example.json"));
    assert!(manifest
        .files
        .iter()
        .any(|file| file == "receipts/run.json"));
    assert_eq!(exported_files(&output), exported_files(&package));
}

#[test]
fn repeated_exports_have_the_same_manifest_and_bytes() {
    let (temp, package) = minimal_package();
    let first_output = temp.path().join("first");
    let second_output = temp.path().join("second");
    let loaded = SkillPackage::load(&package).expect("package");

    let first = GenericExporter
        .export(&loaded, &first_output)
        .expect("first export");
    let second = GenericExporter
        .export(&loaded, &second_output)
        .expect("second export");

    assert_eq!(first, second);
    assert_eq!(
        exported_files(&first_output),
        exported_files(&second_output)
    );
}

#[test]
fn lint_failure_does_not_create_an_output_directory() {
    let (temp, package) = minimal_package();
    fs::write(
        package.join("workflow.yaml"),
        "schema: skilltape.dev/workflow/v1\nsteps:\n  - action: exec\n    id: blocked\n    program: python\n    args: []\n    timeout_ms: 1000\n",
    )
    .expect("workflow");
    let output = temp.path().join("exported");
    let loaded = SkillPackage::load(&package).expect("package loads");

    let error = GenericExporter
        .export(&loaded, &output)
        .expect_err("lint failure");
    assert!(matches!(error, ExportError::Lint { .. }));
    assert!(!output.exists());
}

#[test]
fn existing_and_unsafe_outputs_are_rejected_without_overwrite() {
    let (temp, package) = minimal_package();
    let loaded = SkillPackage::load(&package).expect("package");
    let output = temp.path().join("existing");
    fs::create_dir(&output).expect("output");
    fs::write(output.join("keep.txt"), "keep").expect("marker");

    let error = GenericExporter
        .export(&loaded, &output)
        .expect_err("existing output");
    assert!(matches!(error, ExportError::OutputExists { .. }));
    assert_eq!(
        fs::read_to_string(output.join("keep.txt")).expect("marker"),
        "keep"
    );

    let unsafe_output = temp.path().join("../outside-export");
    let error = GenericExporter
        .export(&loaded, &unsafe_output)
        .expect_err("unsafe output");
    assert!(matches!(error, ExportError::UnsafeOutput { .. }));
}

#[cfg(unix)]
#[test]
fn symlinked_output_parent_and_source_are_rejected() {
    let (temp, package) = minimal_package();
    let loaded = SkillPackage::load(&package).expect("package");
    let outside = temp.path().join("outside");
    let linked_parent = temp.path().join("linked-parent");
    fs::create_dir(&outside).expect("outside");
    std::os::unix::fs::symlink(&outside, &linked_parent).expect("parent symlink");
    let error = GenericExporter
        .export(&loaded, &linked_parent.join("exported"))
        .expect_err("parent symlink");
    assert!(matches!(error, ExportError::UnsafeOutput { .. }));
    assert!(!outside.join("exported").exists());

    fs::create_dir(package.join("fixtures")).expect("fixtures");
    std::os::unix::fs::symlink(&outside, package.join("fixtures/link")).expect("source symlink");
    let loaded = SkillPackage::load(&package).expect("package with optional symlink");
    let error = GenericExporter
        .export(&loaded, &temp.path().join("source-export"))
        .expect_err("source symlink");
    assert!(matches!(error, ExportError::SymlinkSource { .. }));
}

#[cfg(unix)]
#[test]
fn referenced_script_permissions_are_preserved() {
    use std::os::unix::fs::PermissionsExt;

    let (temp, package) = script_package();
    let output = temp.path().join("exported");
    let loaded = SkillPackage::load(&package).expect("package");
    GenericExporter.export(&loaded, &output).expect("export");
    let mode = fs::metadata(output.join("scripts/run.sh"))
        .expect("exported script")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o755);
}
