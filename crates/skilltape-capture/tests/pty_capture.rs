use std::path::Path;

use skilltape_capture::{capture_terminal, CaptureOptions};
use skilltape_tape::{TapeEventKind, TapeManifest, TapeStore, TAPE_SCHEMA_V1};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

fn store(root: &Path, workspace: &Path) -> TapeStore {
    TapeStore::create(
        root,
        TapeManifest {
            schema: TAPE_SCHEMA_V1.to_owned(),
            id: "capture-test".to_owned(),
            started_at_ms: 1,
            finished_at_ms: None,
            platform: std::env::consts::OS.to_owned(),
            workspace_root: workspace
                .file_name()
                .expect("workspace name")
                .to_string_lossy()
                .into_owned(),
            event_count: 0,
        },
    )
    .expect("create tape store")
}

#[tokio::test]
async fn portable_pty_captures_a_temp_script_without_persisting_secrets() {
    let temp = TempDir::new().expect("temp directory");
    let workspace = temp.path().join("workspace");
    std::fs::create_dir(&workspace).expect("workspace");
    let script = workspace.join("capture.sh");
    std::fs::write(
        &script,
        "printf 'hello from stdout\\n'; printf 'password=super-secret-value\\n' >&2; exit 7\n",
    )
    .expect("write script");
    let tape_root = temp.path().join("tape");
    let store = store(&tape_root, &workspace);

    let summary = capture_terminal(
        CaptureOptions {
            command: "/bin/sh".to_owned(),
            args: vec![script.to_string_lossy().into_owned()],
            workspace: workspace.clone(),
            env_allowlist: vec![],
            output_limit: 4096,
        },
        store,
        CancellationToken::new(),
    )
    .await
    .expect("capture succeeds");

    assert_eq!(summary.exit_code, 7);
    assert!(!summary.cancelled);
    assert!(!summary.output_truncated);

    let reopened = TapeStore::open(&tape_root).expect("reopen tape");
    let events = reopened
        .read_events()
        .expect("read events")
        .collect::<Result<Vec<_>, _>>()
        .expect("valid events");
    assert_eq!(events.len(), 4);
    assert_eq!(events[0].kind, TapeEventKind::SessionStarted);
    assert_eq!(events[1].payload["phase"], "started");
    assert_eq!(events[2].payload["phase"], "output");
    assert_eq!(events[3].kind, TapeEventKind::SessionFinished);
    assert!(events[2].payload["text"]
        .as_str()
        .expect("output text")
        .contains("hello from stdout"));
    assert_eq!(events[2].payload["stdout_stderr_merged"], true);

    let persisted =
        std::fs::read_to_string(tape_root.join("events.jsonl")).expect("persisted events");
    assert!(!persisted.contains("super-secret-value"));
    assert!(persisted.contains("[REDACTED"));
}

#[tokio::test]
async fn capture_respects_the_output_limit() {
    let temp = TempDir::new().expect("temp directory");
    let workspace = temp.path().join("workspace");
    std::fs::create_dir(&workspace).expect("workspace");
    let tape_root = temp.path().join("tape");
    let store = store(&tape_root, &workspace);

    let summary = capture_terminal(
        CaptureOptions {
            command: "/usr/bin/printf".to_owned(),
            args: vec!["1234567890".to_owned()],
            workspace,
            env_allowlist: vec![],
            output_limit: 5,
        },
        store,
        CancellationToken::new(),
    )
    .await
    .expect("capture succeeds");

    assert!(summary.output_truncated);
    let events = TapeStore::open(&tape_root)
        .expect("reopen tape")
        .read_events()
        .expect("read events")
        .collect::<Result<Vec<_>, _>>()
        .expect("valid events");
    assert_eq!(events[1].payload["command"], "/usr/bin/printf");
    assert!(events[2].payload["text"].as_str().expect("text").len() <= 5);
}

#[tokio::test]
async fn cancellation_terminates_and_reaps_the_child() {
    let temp = TempDir::new().expect("temp directory");
    let workspace = temp.path().join("workspace");
    std::fs::create_dir(&workspace).expect("workspace");
    let tape_root = temp.path().join("tape");
    let store = store(&tape_root, &workspace);
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();

    let capture = tokio::spawn(capture_terminal(
        CaptureOptions {
            command: "/bin/sh".to_owned(),
            args: vec!["-c".to_owned(), "while :; do :; done".to_owned()],
            workspace,
            env_allowlist: vec![],
            output_limit: 1024,
        },
        store,
        cancel,
    ));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    trigger.cancel();

    let summary = tokio::time::timeout(std::time::Duration::from_secs(3), capture)
        .await
        .expect("cancelled capture must not hang")
        .expect("capture task joins")
        .expect("capture records cancellation");
    assert!(summary.cancelled);

    let manifest = TapeStore::open(&tape_root)
        .expect("reopen tape")
        .read_manifest()
        .expect("manifest");
    assert!(manifest.finished_at_ms.is_some());
}
