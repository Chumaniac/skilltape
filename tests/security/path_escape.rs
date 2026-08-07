use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;
use assert_cmd::Command;
use skilltape_core::{create_skill_template, SkillPackage};
use skilltape_policy::{codes, FileAccess, PolicyEngine};
use tempfile::TempDir;

#[test]
fn cli_rejects_capture_output_with_parent_component_before_running_command() {
    let temp = TempDir::new().expect("temporary directory");
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let escaped = temp.path().join("nested").join("..").join("escape-tape");

    let mut command = Command::cargo_bin("skilltape").expect("skilltape binary");
    command
        .current_dir(&workspace)
        .args(["capture", "escape", "--command", "/bin/echo", "--output"])
        .arg(&escaped)
        .args(["--yes"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("unsafe"));

    assert!(!temp.path().join("escape-tape").exists());
}

#[test]
fn cli_rejects_compile_output_with_parent_component_before_reading_tape() {
    let temp = TempDir::new().expect("temporary directory");
    let escaped = temp.path().join("nested").join("..").join("escape-skill");

    let mut command = Command::cargo_bin("skilltape").expect("skilltape binary");
    command
        .args(["compile", "missing-tape", "--output"])
        .arg(&escaped)
        .assert()
        .code(2)
        .stderr(predicates::str::contains("unsafe"));

    assert!(!temp.path().join("escape-skill").exists());
}

#[test]
fn policy_rejects_path_command_network_and_secret_bypass_attempts() {
    let temp = TempDir::new().expect("temporary directory");
    let package_root = temp.path().join("package");
    create_skill_template(&package_root, "security-policy").expect("skill template");
    let package = SkillPackage::load(&package_root).expect("load package");
    let engine = PolicyEngine::default();

    let path = engine.check_path("../outside.txt", FileAccess::Write, &package.permissions);
    assert!(!path.allowed);
    assert_eq!(path.code, codes::UNSAFE_PATH);
    assert!(!path.reason.contains("outside.txt"));

    let command = engine.check_command(
        "sh",
        &["-c".to_owned(), "echo injected".to_owned()],
        &package.permissions,
    );
    assert!(!command.allowed);
    assert_eq!(command.code, codes::DANGEROUS_COMMAND);
    assert!(!command.reason.contains("injected"));

    let network = engine.check_network("example.com", &package.permissions);
    assert!(!network.allowed);
    assert_eq!(network.code, codes::NETWORK_DISABLED);

    let mut allowlisted = package.permissions.clone();
    allowlisted.network.enabled = true;
    allowlisted.network.allow_hosts = vec!["allowed.example".to_owned()];
    let bypass = engine.check_network("allowed.example.attacker.test", &allowlisted);
    assert!(!bypass.allowed);
    assert_eq!(bypass.code, codes::HOST_NOT_ALLOWED);

    allowlisted.secrets.read_environment = true;
    let secret = engine.check_environment("SKILLTAPE_SYNTHETIC_SECRET", &allowlisted);
    assert!(!secret.allowed);
    assert_eq!(secret.code, codes::SECRET_IDENTIFIER);
    assert!(!secret.reason.contains("SKILLTAPE_SYNTHETIC_SECRET"));
}

#[cfg(unix)]
#[test]
fn cli_rejects_receipt_parent_symlink_without_publishing_outside_root() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().expect("temporary directory");
    let package_root = temp.path().join("package");
    let outside = temp.path().join("outside");
    let linked = temp.path().join("linked");
    create_skill_template(&package_root, "receipt-safety").expect("skill template");
    fs::create_dir(&outside).expect("outside directory");
    symlink(&outside, &linked).expect("receipt symlink");

    let receipt = linked.join("run.json");
    let mut command = Command::cargo_bin("skilltape").expect("skilltape binary");
    command
        .args(["verify"])
        .arg(&package_root)
        .args(["--receipt"])
        .arg(&receipt)
        .args(["--json"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("unsafe"));

    assert!(!outside.join("run.json").exists());
}

#[cfg(unix)]
#[test]
fn capture_cancellation_kills_background_process_group() {
    let temp = TempDir::new().expect("temporary directory");
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let ready = workspace.join("ready");
    let child_pid_file = workspace.join("child.pid");
    let script = workspace.join("spawn-child.sh");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf ready > '{}'\n/bin/sh -c 'trap \"\" INT TERM; /bin/sleep 30' &\necho $! > '{}'\nwait\n",
            ready.display(),
            child_pid_file.display()
        ),
    )
    .expect("background script");
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(&script)
        .expect("script metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).expect("script permissions");
    let tape = temp.path().join("tape");

    let mut command = std::process::Command::cargo_bin("skilltape").expect("skilltape binary");
    let mut child = command
        .current_dir(&workspace)
        .args(["capture", "background", "--command"])
        .arg(&script)
        .args(["--output"])
        .arg(&tape)
        .args(["--yes"])
        .spawn()
        .expect("capture process");

    let deadline = Instant::now() + Duration::from_secs(5);
    while (!ready.is_file() || !child_pid_file.is_file()) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    if !ready.is_file() || !child_pid_file.is_file() {
        unsafe {
            libc::kill(child.id() as libc::pid_t, libc::SIGKILL);
        }
        let _ = child.wait();
        panic!("capture did not start the background process");
    }

    let descendant_pid = fs::read_to_string(&child_pid_file)
        .expect("descendant pid")
        .trim()
        .parse::<libc::pid_t>()
        .expect("numeric descendant pid");
    let signal_result = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) };
    assert_eq!(signal_result, 0, "send cancellation to capture");

    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().expect("capture status") {
            break status;
        }
        if Instant::now() >= deadline {
            unsafe {
                libc::kill(child.id() as libc::pid_t, libc::SIGKILL);
            }
            let _ = child.wait();
            panic!("cancelled capture did not exit");
        }
        thread::sleep(Duration::from_millis(20));
    };
    assert!(!status.success());

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let result = unsafe { libc::kill(descendant_pid, 0) };
        if result == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
            break;
        }
        if Instant::now() >= deadline {
            unsafe {
                libc::kill(descendant_pid, libc::SIGKILL);
            }
            panic!("background descendant survived cancellation");
        }
        thread::sleep(Duration::from_millis(20));
    }
}
