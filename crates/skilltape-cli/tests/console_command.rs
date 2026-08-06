use std::fs;
use std::net::TcpListener;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;
use tempfile::TempDir;

fn skilltape() -> Command {
    Command::cargo_bin("skilltape").expect("skilltape binary")
}

fn write_file(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write fixture file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).expect("fixture metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("fixture permissions");
    }
}

#[test]
fn console_rejects_missing_workspace_without_starting_a_child() {
    let temp = TempDir::new().expect("temporary directory");
    let output = skilltape()
        .args(["console", "--workspace"])
        .arg(temp.path().join("missing"))
        .output()
        .expect("run console");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("workspace is not a directory"));
}

#[test]
fn console_rejects_an_occupied_localhost_port_before_spawning() {
    let temp = TempDir::new().expect("temporary directory");
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let listener = TcpListener::bind("127.0.0.1:0").expect("occupied listener");
    let marker = temp.path().join("child-started");
    let api = temp.path().join("api.sh");
    write_file(
        &api,
        "#!/bin/sh\nprintf started > \"$SKILLTAPE_CONSOLE_TEST_MARKER\"\n",
    );
    let dist = temp.path().join("dist");
    fs::create_dir(&dist).expect("dist");
    fs::write(dist.join("index.html"), "<main>Console</main>").expect("index");

    let output = skilltape()
        .args(["console", "--workspace"])
        .arg(&workspace)
        .args([
            "--port",
            &listener.local_addr().expect("address").port().to_string(),
        ])
        .env("SKILLTAPE_CONSOLE_API_BIN", &api)
        .env("SKILLTAPE_CONSOLE_UI_DIST", &dist)
        .env("SKILLTAPE_CONSOLE_TEST_MARKER", &marker)
        .output()
        .expect("run console");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("port is unavailable on localhost"));
    assert!(!marker.exists());
}

#[cfg(unix)]
#[test]
fn console_uses_loopback_waits_for_readiness_opens_and_reaps_the_api_child() {
    let temp = TempDir::new().expect("temporary directory");
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let dist = temp.path().join("dist");
    fs::create_dir(&dist).expect("dist");
    fs::write(dist.join("index.html"), "<main>Console</main>").expect("index");
    let arguments = temp.path().join("api-arguments");
    let opened = temp.path().join("opened-url");
    let api = temp.path().join("api.sh");
    write_file(
        &api,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$SKILLTAPE_CONSOLE_TEST_ARGUMENTS\"\nport=0\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--port\" ]; then port=$2; shift 2; continue; fi\n  shift\ndone\nprintf 'SkillTape Console API listening at http://127.0.0.1:%s\\n' \"$port\"\ntrap 'exit 0' INT TERM\nwhile :; do sleep 1; done\n",
    );
    let opener = temp.path().join("open.sh");
    write_file(
        &opener,
        "#!/bin/sh\nprintf '%s' \"$1\" > \"$SKILLTAPE_CONSOLE_TEST_OPENED\"\n",
    );

    let mut child = skilltape()
        .args(["console", "--workspace"])
        .arg(&workspace)
        .args(["--port", "0", "--open"])
        .env("SKILLTAPE_CONSOLE_API_BIN", &api)
        .env("SKILLTAPE_CONSOLE_UI_DIST", &dist)
        .env("SKILLTAPE_CONSOLE_OPEN_COMMAND", &opener)
        .env("SKILLTAPE_CONSOLE_TEST_ARGUMENTS", &arguments)
        .env("SKILLTAPE_CONSOLE_TEST_OPENED", &opened)
        .spawn()
        .expect("spawn console");

    let deadline = Instant::now() + Duration::from_secs(5);
    while !opened.exists() {
        assert!(Instant::now() < deadline, "console did not become ready");
        std::thread::sleep(Duration::from_millis(10));
    }

    let arguments = fs::read_to_string(&arguments).expect("API arguments");
    assert!(arguments.lines().any(|value| value == "127.0.0.1"));
    assert!(arguments.lines().any(|value| value == "--static-root"));
    assert_eq!(
        fs::read_to_string(&opened).expect("opened URL"),
        "http://127.0.0.1:0"
    );

    let status = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .expect("send interrupt");
    assert!(status.success());
    assert!(!child.wait().expect("console exit").success());
}
