use std::fs;
use std::path::{Path, PathBuf};

use skilltape_core::create_skill_template;
use skilltape_export::{
    run_plugin, validate_plugin_manifest, validate_request, ExportRequest, PluginError,
    PluginExportManifest, EXPORT_MANIFEST_SCHEMA_V1,
};
use tempfile::TempDir;

const PACKAGE_FILES: [&str; 6] = [
    "skilltape.yaml",
    "workflow.yaml",
    "permissions.json",
    "skilltape.lock",
    "SKILL.md",
    "README.md",
];

fn fixture() -> (TempDir, PathBuf, PathBuf, ExportRequest) {
    let temp = TempDir::new().expect("temp directory");
    let input = temp.path().join("input");
    let output = temp.path().join("output");
    create_skill_template(&input, "plugin-test").expect("skill template");
    let request = ExportRequest::new(
        "test-target",
        input.to_string_lossy(),
        output.to_string_lossy(),
        "a".repeat(64),
        vec!["metadata".to_owned()],
    );
    (temp, input, output, request)
}

fn manifest(request: &ExportRequest) -> PluginExportManifest {
    PluginExportManifest {
        schema: EXPORT_MANIFEST_SCHEMA_V1.to_owned(),
        target: request.target.clone(),
        package_path: ".".to_owned(),
        files: PACKAGE_FILES
            .iter()
            .map(|file| (*file).to_owned())
            .collect(),
        package_hash: request.package_hash.clone(),
        capabilities: request.required_capabilities.clone(),
    }
}

fn copy_package(source: &Path, output: &Path) {
    fs::create_dir_all(output).expect("output directory");
    for file in PACKAGE_FILES {
        fs::copy(source.join(file), output.join(file)).expect("copy package file");
    }
}

#[test]
fn unknown_request_schema_is_rejected_before_process_launch() {
    let (_temp, _input, _output, mut request) = fixture();
    request.schema = "skilltape.dev/export-request/v9".to_owned();

    assert!(matches!(
        validate_request(&request),
        Err(PluginError::UnsupportedRequestSchema { .. })
    ));
}

#[test]
fn missing_capability_is_rejected_before_linting_output() {
    let (_temp, input, output, request) = fixture();
    copy_package(&input, &output);
    let mut plugin_manifest = manifest(&request);
    plugin_manifest.capabilities.clear();

    assert!(matches!(
        validate_plugin_manifest(&request, &plugin_manifest, &output),
        Err(PluginError::MissingCapability { capability }) if capability == "metadata"
    ));
}

#[test]
fn manifest_paths_cannot_escape_the_requested_output_root() {
    let (_temp, _input, output, request) = fixture();
    fs::create_dir_all(&output).expect("output directory");
    let mut plugin_manifest = manifest(&request);
    plugin_manifest.files = vec!["../outside.txt".to_owned()];

    assert!(matches!(
        validate_plugin_manifest(&request, &plugin_manifest, &output),
        Err(PluginError::UnsafeManifestPath { path }) if path == "../outside.txt"
    ));
}

#[test]
fn host_re_lints_a_valid_plugin_package() {
    let (_temp, input, output, request) = fixture();
    copy_package(&input, &output);

    validate_plugin_manifest(&request, &manifest(&request), &output)
        .expect("valid plugin package should pass the host lint gate");
}

#[test]
fn host_rejects_a_plugin_package_with_lint_errors() {
    let (_temp, input, output, request) = fixture();
    copy_package(&input, &output);
    fs::write(
        output.join("workflow.yaml"),
        "schema: skilltape.dev/workflow/v1\nsteps:\n  - id: unsafe\n    action: exec\n    program: python\n    args: []\n    timeout_ms: 1000\n",
    )
    .expect("invalid workflow");

    assert!(matches!(
        validate_plugin_manifest(&request, &manifest(&request), &output),
        Err(PluginError::LintFailed { errors }) if errors > 0
    ));
}

#[cfg(unix)]
#[test]
fn crashed_plugin_is_isolated_and_does_not_publish_a_manifest() {
    use std::os::unix::fs::PermissionsExt;

    let (_temp, input, output, request) = fixture();
    fs::create_dir_all(&input).expect("input directory");
    let plugin = output.parent().expect("parent").join("crash-plugin.sh");
    fs::write(&plugin, "#!/bin/sh\nexit 42\n").expect("plugin script");
    let mut permissions = fs::metadata(&plugin)
        .expect("plugin metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&plugin, permissions).expect("plugin permissions");

    assert!(matches!(
        run_plugin(&plugin, &request),
        Err(PluginError::PluginCrashed { code: Some(42) })
    ));
    assert!(!output.join("skilltape.yaml").exists());
}

#[cfg(unix)]
#[test]
fn output_parent_symlink_is_rejected_before_plugin_launch() {
    let (_temp, input, _output, mut request) = fixture();
    let outside = input.parent().expect("parent").join("outside");
    let linked_parent = input.parent().expect("parent").join("linked-output");
    fs::create_dir(&outside).expect("outside directory");
    std::os::unix::fs::symlink(&outside, &linked_parent).expect("output parent symlink");
    request.output_root = linked_parent
        .join("exported")
        .to_string_lossy()
        .into_owned();

    assert!(matches!(
        run_plugin(Path::new("/bin/true"), &request),
        Err(PluginError::UnsafeOutputRoot { .. })
    ));
    assert!(!outside.join("exported").exists());
}
