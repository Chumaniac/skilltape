use std::future::{poll_fn, Future};
use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};
use skilltape_capture::{
    merge_capture_timeline, watch_workspace, FilesystemCaptureError, FilesystemChange,
    FilesystemChangeKind, TimelineEvent, TimelineFilesystemChange,
};
use skilltape_tape::{EventSource, RedactionState, TapeEvent, TapeEventKind};
use tempfile::TempDir;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);

struct RunningWatcher {
    events: mpsc::Receiver<FilesystemChange>,
    cancel: CancellationToken,
    task: JoinHandle<Result<(), FilesystemCaptureError>>,
}

impl RunningWatcher {
    async fn start(root: &Path) -> Self {
        let (tx, events) = mpsc::channel(64);
        let cancel = CancellationToken::new();
        let task_cancel = cancel.clone();
        let root = root.to_owned();
        let watched_root = root.clone();
        let (ready_tx, ready_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let future = watch_workspace(&watched_root, tx, task_cancel);
            tokio::pin!(future);
            let mut ready_tx = Some(ready_tx);
            poll_fn(|context| {
                let result = future.as_mut().poll(context);
                if let Some(ready_tx) = ready_tx.take() {
                    let _ = ready_tx.send(());
                }
                result
            })
            .await
        });
        ready_rx
            .await
            .expect("watcher reaches its first await during setup");
        let mut watcher = Self {
            events,
            cancel,
            task,
        };

        std::fs::write(root.join("watcher-ready"), b"ready").expect("write readiness marker");
        watcher
            .next_matching(|event| {
                event.kind == FilesystemChangeKind::Created && event.path == "watcher-ready"
            })
            .await;
        std::fs::remove_file(root.join("watcher-ready")).expect("remove readiness marker");
        watcher
            .next_matching(|event| {
                event.kind == FilesystemChangeKind::Deleted && event.path == "watcher-ready"
            })
            .await;
        watcher
    }

    async fn next_matching(
        &mut self,
        predicate: impl Fn(&FilesystemChange) -> bool,
    ) -> FilesystemChange {
        timeout(EVENT_TIMEOUT, async {
            loop {
                let event = tokio::select! {
                    event = self.events.recv() => event.expect("watcher channel stays open"),
                    result = &mut self.task => panic!("watcher exited before event: {result:?}"),
                };
                if predicate(&event) {
                    return event;
                }
            }
        })
        .await
        .expect("expected filesystem event before timeout")
    }

    async fn stop(self) {
        self.cancel.cancel();
        timeout(EVENT_TIMEOUT, self.task)
            .await
            .expect("watcher stops promptly after cancellation")
            .expect("watcher task joins")
            .expect("watcher shuts down cleanly");
    }
}

#[tokio::test]
async fn captures_create_modify_move_and_delete_with_metadata() {
    let temp = TempDir::new().expect("temp directory");
    let root = temp.path().join("workspace");
    std::fs::create_dir(&root).expect("workspace");
    let mut watcher = RunningWatcher::start(&root).await;

    let original = root.join("notes.txt");
    std::fs::write(&original, b"one").expect("create file");
    let created = watcher
        .next_matching(|event| {
            event.kind == FilesystemChangeKind::Created && event.path == "notes.txt"
        })
        .await;
    assert_eq!(created.previous_path, None);
    assert_eq!(created.size, Some(3));
    assert_eq!(
        created.content_hash.as_deref(),
        Some(sha256(b"one").as_str())
    );

    std::fs::write(&original, b"updated").expect("modify file");
    let modified = watcher
        .next_matching(|event| {
            event.kind == FilesystemChangeKind::Modified && event.path == "notes.txt"
        })
        .await;
    assert_eq!(modified.size, Some(7));
    assert_eq!(
        modified.content_hash.as_deref(),
        Some(sha256(b"updated").as_str())
    );

    let moved = root.join("archive.txt");
    std::fs::rename(&original, &moved).expect("rename file");
    let moved_event = watcher
        .next_matching(|event| {
            event.kind == FilesystemChangeKind::Moved && event.path == "archive.txt"
        })
        .await;
    assert_eq!(moved_event.previous_path.as_deref(), Some("notes.txt"));
    assert_eq!(moved_event.size, Some(7));
    assert_eq!(
        moved_event.content_hash.as_deref(),
        Some(sha256(b"updated").as_str())
    );

    std::fs::remove_file(&moved).expect("delete file");
    let deleted = watcher
        .next_matching(|event| {
            event.kind == FilesystemChangeKind::Deleted && event.path == "archive.txt"
        })
        .await;
    assert_eq!(deleted.previous_path, None);
    assert_eq!(deleted.content_hash, None);
    assert_eq!(deleted.size, None);

    watcher.stop().await;
}

#[tokio::test]
async fn normalizes_nested_paths_and_emits_each_duplicate_modify_once() {
    let temp = TempDir::new().expect("temp directory");
    let root = temp.path().join("workspace");
    std::fs::create_dir_all(root.join("nested")).expect("workspace");
    let file = root.join("nested/item.txt");
    std::fs::write(&file, b"before").expect("seed file");
    let mut watcher = RunningWatcher::start(&root).await;

    std::fs::write(&file, b"after").expect("modify file");
    let event = watcher
        .next_matching(|event| {
            event.kind == FilesystemChangeKind::Modified && event.path == "nested/item.txt"
        })
        .await;
    assert!(!event.path.contains('\\'));

    std::fs::write(root.join("zz-sync"), b"sync").expect("write synchronization marker");
    watcher.next_matching(|event| event.path == "zz-sync").await;
    let mut duplicate_count = 1;
    while let Ok(event) = watcher.events.try_recv() {
        if event.kind == FilesystemChangeKind::Modified && event.path == "nested/item.txt" {
            duplicate_count += 1;
        }
    }
    assert_eq!(duplicate_count, 1);

    watcher.stop().await;
}

#[tokio::test]
async fn reports_stable_path_order_for_one_change_batch() {
    let temp = TempDir::new().expect("temp directory");
    let root = temp.path().join("workspace");
    std::fs::create_dir(&root).expect("workspace");
    let mut watcher = RunningWatcher::start(&root).await;

    std::fs::write(root.join("z-last.txt"), b"z").expect("write z");
    std::fs::write(root.join("a-first.txt"), b"a").expect("write a");
    let first = watcher
        .next_matching(|event| {
            event.kind == FilesystemChangeKind::Created
                && matches!(event.path.as_str(), "a-first.txt" | "z-last.txt")
        })
        .await;
    let second = watcher
        .next_matching(|event| {
            event.kind == FilesystemChangeKind::Created
                && matches!(event.path.as_str(), "a-first.txt" | "z-last.txt")
        })
        .await;

    assert_eq!([first.path, second.path], ["a-first.txt", "z-last.txt"]);
    watcher.stop().await;
}

#[tokio::test]
async fn rejects_a_root_that_cannot_contain_workspace_events() {
    let temp = TempDir::new().expect("temp directory");
    let root_file = temp.path().join("not-a-directory");
    std::fs::write(&root_file, b"file").expect("root file");
    let (tx, _rx) = mpsc::channel(1);

    let error = watch_workspace(&root_file, tx, CancellationToken::new())
        .await
        .expect_err("non-directory roots must be rejected");

    assert!(matches!(
        error,
        FilesystemCaptureError::InvalidRoot(path) if path == root_file
    ));
}

#[tokio::test]
async fn cancellation_stops_an_idle_watcher_and_closes_its_channel() {
    let temp = TempDir::new().expect("temp directory");
    let root = temp.path().join("workspace");
    std::fs::create_dir(&root).expect("workspace");
    let mut watcher = RunningWatcher::start(&root).await;

    watcher.cancel.cancel();
    timeout(EVENT_TIMEOUT, &mut watcher.task)
        .await
        .expect("idle watcher stops on cancellation")
        .expect("watcher task joins")
        .expect("watcher returns success");
    assert_eq!(watcher.events.recv().await, None);
}

#[test]
fn merges_filesystem_and_tape_events_within_the_requested_window() {
    let timeline = merge_capture_timeline(
        [TimelineFilesystemChange {
            occurred_at_ms: 1_000,
            change: change(FilesystemChangeKind::Created, "notes.txt"),
        }],
        [tape_event(1_025, 2)],
        Duration::from_millis(25),
    );

    assert_eq!(timeline.len(), 1);
    assert_eq!(timeline[0].start_at_ms, 1_000);
    assert!(matches!(
        timeline[0].events[0],
        TimelineEvent::Filesystem(_)
    ));
    assert!(matches!(timeline[0].events[1], TimelineEvent::Tape(_)));
}

#[test]
fn starts_a_new_timeline_batch_outside_the_requested_window() {
    let timeline = merge_capture_timeline(
        [TimelineFilesystemChange {
            occurred_at_ms: 1_000,
            change: change(FilesystemChangeKind::Created, "notes.txt"),
        }],
        [tape_event(1_026, 2)],
        Duration::from_millis(25),
    );

    assert_eq!(timeline.len(), 2);
    assert_eq!(timeline[0].events.len(), 1);
    assert_eq!(timeline[1].events.len(), 1);
}

#[test]
fn orders_tied_events_by_source_and_stable_event_key() {
    let timeline = merge_capture_timeline(
        [
            TimelineFilesystemChange {
                occurred_at_ms: 1_000,
                change: change(FilesystemChangeKind::Created, "z-last.txt"),
            },
            TimelineFilesystemChange {
                occurred_at_ms: 1_000,
                change: change(FilesystemChangeKind::Created, "a-first.txt"),
            },
        ],
        [tape_event(1_000, 1), tape_event(1_000, 0)],
        Duration::ZERO,
    );

    let labels = timeline[0]
        .events
        .iter()
        .map(|event| match event {
            TimelineEvent::Filesystem(event) => event.change.path.clone(),
            TimelineEvent::Tape(event) => format!("tape-{}", event.sequence),
        })
        .collect::<Vec<_>>();
    assert_eq!(labels, ["a-first.txt", "z-last.txt", "tape-0", "tape-1"]);
}

#[test]
fn orders_mixed_filesystem_kinds_deterministically() {
    let timeline = merge_capture_timeline(
        [
            TimelineFilesystemChange {
                occurred_at_ms: 1_000,
                change: change(FilesystemChangeKind::Deleted, "same.txt"),
            },
            TimelineFilesystemChange {
                occurred_at_ms: 1_000,
                change: change(FilesystemChangeKind::Created, "same.txt"),
            },
            TimelineFilesystemChange {
                occurred_at_ms: 1_000,
                change: change(FilesystemChangeKind::Modified, "same.txt"),
            },
        ],
        [],
        Duration::ZERO,
    );

    assert_eq!(
        timeline[0]
            .events
            .iter()
            .map(|event| match event {
                TimelineEvent::Filesystem(event) => event.change.kind,
                TimelineEvent::Tape(_) => panic!("unexpected tape event"),
            })
            .collect::<Vec<_>>(),
        vec![
            FilesystemChangeKind::Created,
            FilesystemChangeKind::Modified,
            FilesystemChangeKind::Deleted,
        ]
    );
}

fn change(kind: FilesystemChangeKind, path: &str) -> FilesystemChange {
    FilesystemChange {
        kind,
        path: path.to_owned(),
        previous_path: None,
        content_hash: None,
        size: None,
    }
}

fn tape_event(occurred_at_ms: u64, sequence: u64) -> TapeEvent {
    TapeEvent {
        sequence,
        occurred_at_ms,
        kind: TapeEventKind::TerminalCommand,
        source: EventSource::Shell,
        payload: serde_json::json!({"sequence": sequence}),
        redaction: RedactionState::Unredacted,
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
