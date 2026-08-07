use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use assert_cmd::Command;
use serde_json::{json, Value};
use skilltape_core::create_skill_template;
use tempfile::TempDir;

fn executable_script(workspace: &Path, name: &str, body: &str) -> PathBuf {
    let path = workspace.join(name);
    fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n")).expect("script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&path).expect("script metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("script permissions");
    }
    path
}

fn write_echo_package(root: &Path, raw_output: &str) {
    create_skill_template(root, "receipt-secret-safety").expect("skill template");
    fs::write(
        root.join("workflow.yaml"),
        serde_json::to_vec(&json!({
            "schema": "skilltape.dev/workflow/v1",
            "steps": [{
                "action": "exec",
                "id": "emit",
                "program": "/bin/echo",
                "args": [format!("password={raw_output}")],
                "timeout_ms": 1000,
                "outputs": []
            }]
        }))
        .expect("workflow JSON"),
    )
    .expect("workflow");
    fs::write(
        root.join("permissions.json"),
        serde_json::to_vec(&json!({
            "schema": "skilltape.dev/permissions/v1",
            "filesystem": {"read": [], "write": []},
            "process": {
                "executables": ["/bin/echo"],
                "max_processes": 1,
                "default_timeout_ms": 1000
            },
            "network": {"enabled": false, "allow_hosts": []},
            "secrets": {"read_environment": false}
        }))
        .expect("permissions JSON"),
    )
    .expect("permissions");
}

#[cfg(unix)]
#[test]
fn capture_persists_only_secret_free_output_and_environment_metadata() {
    let temp = TempDir::new().expect("temporary directory");
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let variable = format!("SKILLTAPE_ALLOWED_FIXTURE_{}", std::process::id());
    let value = "synthetic-capture-secret-7f4d";
    let script = executable_script(
        &workspace,
        "emit-secret.sh",
        &format!("printf 'password=%s\\n' \"${{{variable}}}\""),
    );
    let tape = temp.path().join("tape");

    let mut command = Command::cargo_bin("skilltape").expect("skilltape binary");
    let result = command
        .current_dir(&workspace)
        .env(&variable, value)
        .args(["capture", "secret-safe", "--command"])
        .arg(&script)
        .args(["--output"])
        .arg(&tape)
        .args(["--allow-env", &variable, "--yes", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();

    let summary = String::from_utf8_lossy(&result.stdout);
    let events = fs::read_to_string(tape.join("events.jsonl")).expect("capture events");
    let observable = format!("{summary}\n{events}");
    assert!(!observable.contains(value));
    assert!(events.contains(&format!("\"name\":\"{variable}\"")));
    assert!(events.contains("\"sha256\":"));
    assert!(!events.contains("environment_value"));
}

#[test]
fn verify_receipt_contains_digests_but_never_raw_command_output() {
    if !replay_sandbox_available() {
        eprintln!("skipping receipt execution test: platform replay sandbox is unavailable");
        return;
    }

    let temp = TempDir::new().expect("temporary directory");
    let package = temp.path().join("package");
    let receipt = temp.path().join("receipts/run.json");
    let raw_output = "synthetic-receipt-secret-83b1";
    write_echo_package(&package, raw_output);

    let mut command = Command::cargo_bin("skilltape").expect("skilltape binary");
    let result = command
        .args(["verify"])
        .arg(&package)
        .args(["--receipt"])
        .arg(&receipt)
        .args(["--json"])
        .assert()
        .success()
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    let written = fs::read_to_string(&receipt).expect("receipt");
    assert!(!stdout.contains(raw_output));
    assert!(!stderr.contains(raw_output));
    assert!(!written.contains(raw_output));

    let receipt_json: Value = serde_json::from_str(&written).expect("receipt JSON");
    let step = &receipt_json["steps"][0];
    assert!(step["stdout_sha256"].as_str().is_some_and(|hash| {
        hash.len() == 64 && hash.chars().all(|character| character.is_ascii_hexdigit())
    }));
    assert_eq!(
        step["stdout_bytes"],
        raw_output.len() + "password=".len() + 1
    );
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
