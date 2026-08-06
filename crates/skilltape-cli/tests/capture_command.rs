use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;
use serde_json::Value;
use tempfile::TempDir;

fn executable_script(workspace: &Path, name: &str, body: &str) -> PathBuf {
    let path = workspace.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&path).expect("script metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("script permissions");
    }
    path
}

fn run_capture(workspace: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    let mut command = assert_cmd::Command::cargo_bin("skilltape").expect("binary");
    command.current_dir(workspace).args(args);
    command.assert()
}

#[test]
fn capture_uses_the_current_directory_and_safe_default_output() {
    let temp = TempDir::new().expect("temp directory");
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let script = executable_script(&workspace, "capture.sh", "printf 'captured output\\n'");

    run_capture(
        &workspace,
        &[
            "capture",
            "default-demo",
            "--command",
            script.to_str().expect("script path"),
            "--yes",
        ],
    )
    .success();

    let tape = workspace.join(".skilltape/tapes/default-demo");
    assert!(tape.join("manifest.json").is_file());
    assert!(tape.join("events.jsonl").is_file());
}

#[test]
fn capture_writes_the_requested_output_and_json_summary() {
    let temp = TempDir::new().expect("temp directory");
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let script = executable_script(&workspace, "capture.sh", "printf 'json output\\n'");
    let output = temp.path().join("tapes/json-demo");

    let stdout = run_capture(
        &workspace,
        &[
            "capture",
            "json-demo",
            "--command",
            script.to_str().expect("script path"),
            "--output",
            output.to_str().expect("output path"),
            "--json",
            "--yes",
        ],
    )
    .success()
    .get_output()
    .stdout
    .clone();

    let summary: Value = serde_json::from_slice(&stdout).expect("stable JSON summary");
    assert_eq!(summary["name"], "json-demo");
    assert_eq!(summary["tape_path"], output.to_string_lossy().as_ref());
    assert_eq!(summary["cancelled"], false);
    assert!(summary["event_count"].as_u64().expect("event count") >= 4);
    assert!(output.join("events.jsonl").is_file());
}

#[test]
fn capture_persists_command_output_and_metadata_only_file_changes() {
    let temp = TempDir::new().expect("temp directory");
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let script = executable_script(
        &workspace,
        "capture.sh",
        "printf 'hello from capture\\n'; printf 'password=super-secret-value\\n' >&2; printf created > created.txt",
    );
    let output = workspace.join("tape");

    run_capture(
        &workspace,
        &[
            "capture",
            "metadata-demo",
            "--command",
            script.to_str().expect("script path"),
            "--output",
            output.to_str().expect("output path"),
            "--yes",
        ],
    )
    .success();

    let events = fs::read_to_string(output.join("events.jsonl")).expect("events");
    assert!(events.contains("hello from capture"));
    assert!(events.contains("filesystem_changed"));
    assert!(events.contains("created.txt"));
    assert!(!events.contains("super-secret-value"));
    assert!(!events.contains("created\\\""));
}

#[test]
fn capture_requires_explicit_confirmation_for_commands() {
    let temp = TempDir::new().expect("temp directory");
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let script = executable_script(&workspace, "capture.sh", "printf should-not-run");

    run_capture(
        &workspace,
        &[
            "capture",
            "needs-confirmation",
            "--command",
            script.to_str().expect("script path"),
        ],
    )
    .code(1)
    .stderr(predicates::str::contains("--yes"));

    assert!(!workspace
        .join(".skilltape/tapes/needs-confirmation")
        .exists());
}

#[test]
fn capture_reports_command_errors_with_nonzero_exit_code() {
    let temp = TempDir::new().expect("temp directory");
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");

    run_capture(
        &workspace,
        &[
            "capture",
            "missing-command",
            "--command",
            workspace.join("missing-program").to_str().expect("path"),
            "--yes",
        ],
    )
    .code(1)
    .stderr(predicates::str::contains("capture"));
}

#[cfg(unix)]
#[test]
fn capture_cancels_on_sigint_and_returns_nonzero() {
    let temp = TempDir::new().expect("temp directory");
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let ready = workspace.join("capture.ready");
    let script = executable_script(
        &workspace,
        "capture.sh",
        "printf ready > capture.ready; sleep 10",
    );
    let output = workspace.join("cancelled-tape");

    let mut command = Command::cargo_bin("skilltape").expect("binary");
    let mut child = command
        .current_dir(&workspace)
        .args([
            "capture",
            "cancelled-demo",
            "--command",
            script.to_str().expect("script path"),
            "--output",
            output.to_str().expect("output path"),
            "--yes",
        ])
        .spawn()
        .expect("capture process");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !ready.is_file() {
        assert!(
            std::time::Instant::now() < deadline,
            "capture process did not become ready"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let status = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .expect("send SIGINT");
    assert!(status.success());
    let status = child.wait().expect("capture exit");
    assert!(!status.success());

    let events = fs::read_to_string(output.join("events.jsonl")).expect("cancelled events");
    assert!(events.contains("\"cancelled\":true"));
}

#[test]
fn capture_rejects_unsafe_output_paths() {
    let temp = TempDir::new().expect("temp directory");
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let script = executable_script(&workspace, "capture.sh", "printf should-not-run");

    run_capture(
        &workspace,
        &[
            "capture",
            "unsafe-output",
            "--command",
            script.to_str().expect("script path"),
            "--output",
            "../outside",
            "--yes",
        ],
    )
    .code(1)
    .stderr(predicates::str::contains("unsafe"));
}

#[cfg(unix)]
#[test]
fn capture_rejects_default_output_through_external_symlink() {
    let temp = TempDir::new().expect("temp directory");
    let workspace = temp.path().join("workspace");
    let outside = temp.path().join("outside");
    fs::create_dir(&workspace).expect("workspace");
    fs::create_dir(&outside).expect("outside");
    let script = executable_script(&workspace, "capture.sh", "printf should-not-run");
    std::os::unix::fs::symlink(&outside, workspace.join(".skilltape")).expect("symlink");

    run_capture(
        &workspace,
        &[
            "capture",
            "symlink-escape",
            "--command",
            script.to_str().expect("script path"),
            "--yes",
        ],
    )
    .code(1)
    .stderr(predicates::str::contains("unsafe"));

    assert!(!outside.join("tapes/symlink-escape").exists());
}
