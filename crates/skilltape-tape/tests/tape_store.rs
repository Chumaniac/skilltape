use std::fs;
use std::io::Write;

use skilltape_tape::{
    EventSource, RedactionState, TapeEvent, TapeEventKind, TapeIdGenerator, TapeManifest,
    TapeStore, TapeStoreError, TAPE_SCHEMA_V1,
};
use tempfile::TempDir;

fn manifest(id: &str) -> TapeManifest {
    TapeManifest {
        schema: TAPE_SCHEMA_V1.to_owned(),
        id: id.to_owned(),
        started_at_ms: 10,
        finished_at_ms: None,
        platform: "test".to_owned(),
        workspace_root: "workspace".to_owned(),
        event_count: 0,
    }
}

fn event(sequence: u64) -> TapeEvent {
    TapeEvent {
        sequence,
        occurred_at_ms: 10 + sequence,
        kind: TapeEventKind::TerminalCommand,
        source: EventSource::Cli,
        payload: serde_json::json!({"sequence": sequence}),
        redaction: RedactionState::Unredacted,
    }
}

fn create_store() -> (TempDir, std::path::PathBuf, TapeStore) {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().join("tape-1");
    let store = TapeStore::create(&root, manifest("tape-1")).unwrap();
    (temp_dir, root, store)
}

#[test]
fn creates_empty_tape_and_reopens_it_without_overwriting() {
    let (_temp_dir, root, store) = create_store();

    assert_eq!(store.read_manifest().unwrap(), manifest("tape-1"));
    assert!(store.read_events().unwrap().next().is_none());
    assert!(root.join("manifest.json").is_file());
    assert!(root.join("events.jsonl").is_file());

    let reopened = TapeStore::open(&root).unwrap();
    assert_eq!(reopened.read_manifest().unwrap(), manifest("tape-1"));
    assert!(matches!(
        TapeStore::create(&root, manifest("other")),
        Err(TapeStoreError::AlreadyExists { .. })
    ));
}

#[test]
fn appends_events_with_fsync_and_recovers_them_in_sequence() {
    let (_temp_dir, root, store) = create_store();
    let expected = vec![event(0), event(1)];

    for item in &expected {
        store.append(item).unwrap();
    }

    assert_eq!(store.read_manifest().unwrap().event_count, 2);
    let recovered: Vec<_> = TapeStore::open(root)
        .unwrap()
        .read_events()
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(recovered, expected);
}

#[test]
fn duplicate_sequence_is_rejected_without_changing_existing_events() {
    let (_temp_dir, root, store) = create_store();
    store.append(&event(0)).unwrap();
    let before = fs::read(root.join("events.jsonl")).unwrap();

    assert!(matches!(
        store.append(&event(0)),
        Err(TapeStoreError::SequenceMismatch {
            expected: 1,
            actual: 0,
            ..
        })
    ));
    assert_eq!(fs::read(root.join("events.jsonl")).unwrap(), before);
    assert_eq!(store.read_manifest().unwrap().event_count, 1);
}

#[test]
fn finish_atomically_updates_manifest_and_rejects_later_writes() {
    let (_temp_dir, root, store) = create_store();
    store.append(&event(0)).unwrap();

    let finished = store.finish(99).unwrap();
    assert_eq!(finished.finished_at_ms, Some(99));
    assert_eq!(store.read_manifest().unwrap(), finished);
    assert!(!root.join("manifest.json.tmp").exists());
    assert!(matches!(
        store.append(&event(1)),
        Err(TapeStoreError::AlreadyFinished)
    ));
    assert!(matches!(
        store.finish(100),
        Err(TapeStoreError::AlreadyFinished)
    ));
}

#[test]
fn truncated_jsonl_is_reported_without_discarding_prior_events() {
    let (_temp_dir, root, store) = create_store();
    store.append(&event(0)).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(root.join("events.jsonl"))
        .unwrap()
        .write_all(b"{\"sequence\":1")
        .unwrap();

    let mut recovered = store.read_events().unwrap();
    assert_eq!(recovered.next().unwrap().unwrap(), event(0));
    assert!(matches!(
        recovered.next().unwrap(),
        Err(TapeStoreError::InvalidJsonl { line: 2, .. })
    ));
}

#[test]
fn a_non_contiguous_recovered_sequence_is_rejected() {
    let (_temp_dir, root, store) = create_store();
    store.append(&event(0)).unwrap();
    let mut raw = serde_json::to_string(&event(2)).unwrap();
    raw.push('\n');
    fs::OpenOptions::new()
        .append(true)
        .open(root.join("events.jsonl"))
        .unwrap()
        .write_all(raw.as_bytes())
        .unwrap();

    let mut recovered = store.read_events().unwrap();
    assert_eq!(recovered.next().unwrap().unwrap(), event(0));
    assert!(matches!(
        recovered.next().unwrap(),
        Err(TapeStoreError::SequenceMismatch {
            expected: 1,
            actual: 2,
            line: 2,
        })
    ));
}

#[test]
fn unsafe_roots_and_invalid_manifests_are_rejected() {
    let temp_dir = tempfile::tempdir().unwrap();
    let unsafe_root = temp_dir.path().join("..").join("outside");
    assert!(matches!(
        TapeStore::create(unsafe_root, manifest("unsafe")),
        Err(TapeStoreError::UnsafeRoot { .. })
    ));

    let file_root = temp_dir.path().join("file");
    fs::write(&file_root, b"not a directory").unwrap();
    assert!(matches!(
        TapeStore::create(&file_root, manifest("file")),
        Err(TapeStoreError::InvalidRoot { .. })
    ));

    let mut invalid = manifest("invalid");
    invalid.schema = "skilltape.dev/tape/v2".to_owned();
    assert!(matches!(
        TapeStore::create(temp_dir.path().join("invalid"), invalid),
        Err(TapeStoreError::InvalidManifest { .. })
    ));
}

#[test]
fn local_ids_are_deterministic_and_lexicographically_sortable() {
    let mut first = TapeIdGenerator::new(42);
    let mut second = TapeIdGenerator::new(42);

    let first_id = first.next();
    let second_id = first.next();
    assert_eq!(first_id, second.next());
    assert_eq!(second_id, second.next());
    assert!(first_id < second_id);
    assert_eq!(first_id, "tape_00000000000000000042-00000000000000000000");
}
