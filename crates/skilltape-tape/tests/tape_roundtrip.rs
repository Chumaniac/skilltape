use skilltape_tape::{
    EventSource, RedactionState, TapeEvent, TapeEventKind, TapeManifest, TAPE_SCHEMA_V1,
};

fn event(sequence: u64, kind: TapeEventKind) -> TapeEvent {
    TapeEvent {
        sequence,
        occurred_at_ms: 1,
        kind,
        source: EventSource::Cli,
        payload: serde_json::json!({"ok": true}),
        redaction: RedactionState::Unredacted,
    }
}

#[test]
fn events_round_trip_as_single_line_jsonl() {
    let value = serde_json::to_string(&event(0, TapeEventKind::TerminalCommand)).unwrap();
    assert!(!value.contains('\n'));
    assert_eq!(
        serde_json::from_str::<TapeEvent>(&value).unwrap(),
        event(0, TapeEventKind::TerminalCommand)
    );
}

#[test]
fn manifest_round_trips_and_schema_is_exact() {
    let manifest = TapeManifest {
        schema: TAPE_SCHEMA_V1.into(),
        id: "session-1".into(),
        started_at_ms: 1,
        finished_at_ms: Some(2),
        platform: "macos".into(),
        workspace_root: "workspace".into(),
        event_count: 1,
    };
    let value = serde_json::to_value(&manifest).unwrap();
    assert_eq!(
        serde_json::from_value::<TapeManifest>(value).unwrap(),
        manifest
    );
}

#[test]
fn invalid_values_are_rejected() {
    assert!(serde_json::from_value::<TapeManifest>(serde_json::json!({"schema": TAPE_SCHEMA_V1, "id": "", "started_at_ms": 1, "platform": "macos", "workspace_root": "/tmp/work", "event_count": 0})).is_err());
    assert!(serde_json::from_value::<TapeEvent>(serde_json::json!({"sequence": 0, "occurred_at_ms": -1, "kind": "terminal_command", "source": "cli", "payload": {}, "redaction": "unredacted"})).is_err());
}

#[test]
fn unknown_event_kind_is_rejected() {
    assert!(serde_json::from_value::<TapeEvent>(serde_json::json!({
        "sequence": 0,
        "occurred_at_ms": 1,
        "kind": "future_event",
        "source": "cli",
        "payload": {},
        "redaction": "unredacted"
    }))
    .is_err());
}

#[test]
fn cross_platform_absolute_and_traversal_roots_are_rejected() {
    for root in [
        "/tmp/work",
        "\\\\server\\share",
        "\\work",
        "C:\\work",
        "workspace/../outside",
        "workspace\\..\\outside",
    ] {
        let manifest = serde_json::json!({
            "schema": TAPE_SCHEMA_V1,
            "id": "session-1",
            "started_at_ms": 1,
            "platform": "test",
            "workspace_root": root,
            "event_count": 0
        });
        assert!(
            serde_json::from_value::<TapeManifest>(manifest).is_err(),
            "{root}"
        );
    }
}

#[test]
fn event_sequences_are_monotonic() {
    let events = [
        event(0, TapeEventKind::SessionStarted),
        event(1, TapeEventKind::CaptureWarning),
        event(2, TapeEventKind::SessionFinished),
    ];
    assert!(events
        .windows(2)
        .all(|pair| pair[1].sequence > pair[0].sequence));
    assert!(![
        event(1, TapeEventKind::SessionStarted),
        event(1, TapeEventKind::SessionFinished)
    ]
    .windows(2)
    .all(|pair| pair[1].sequence > pair[0].sequence));
}

#[test]
fn schema_validates_each_event_kind() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../schemas/tape/v1.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for kind in [
        "session_started",
        "session_finished",
        "terminal_command",
        "filesystem_changed",
        "permission_requested",
        "permission_decided",
        "environment_snapshot",
        "capture_warning",
    ] {
        let value = serde_json::to_value(event(
            0,
            serde_json::from_str(&format!("\"{kind}\"")).unwrap(),
        ))
        .unwrap();
        assert!(validator.is_valid(&value), "{kind}");
    }
}
