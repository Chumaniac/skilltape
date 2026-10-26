use std::fs;
use std::path::{Path, PathBuf};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use skilltape_console_api::{router, router_with_static, ConsoleReadModel};
use skilltape_core::create_skill_template;
use skilltape_tape::{
    EventSource, RedactionState, TapeEvent, TapeEventKind, TapeManifest, TapeStore, TAPE_SCHEMA_V1,
};
use tempfile::TempDir;
use tower::ServiceExt;

fn fixture() -> (TempDir, PathBuf) {
    let temp = TempDir::new().expect("temp directory");
    let root = temp.path().join("workspace");
    fs::create_dir(&root).expect("workspace");
    fs::create_dir_all(root.join(".skilltape/tapes")).expect("tape directory");
    let store = TapeStore::create(
        root.join(".skilltape/tapes/tape-a"),
        TapeManifest {
            schema: TAPE_SCHEMA_V1.to_owned(),
            id: "tape-a".to_owned(),
            started_at_ms: 10,
            finished_at_ms: None,
            platform: "test".to_owned(),
            workspace_root: "workspace".to_owned(),
            event_count: 0,
        },
    )
    .expect("tape store");
    store
        .append(&TapeEvent {
            sequence: 0,
            occurred_at_ms: 11,
            kind: TapeEventKind::TerminalCommand,
            source: EventSource::Shell,
            payload: json!({"command": "printf", "output": "redacted-value"}),
            redaction: RedactionState::Redacted,
        })
        .expect("tape event");
    store.finish(12).expect("tape finish");

    let skill = root.join("skills/demo");
    create_skill_template(&skill, "demo").expect("skill package");

    fs::create_dir_all(root.join(".skilltape/runs/run-a")).expect("run directory");
    fs::write(
        root.join(".skilltape/runs/run-a/run.json"),
        serde_json::to_vec(&json!({
            "schema": "skilltape.dev/run/v1",
            "status": "succeeded",
            "steps": []
        }))
        .expect("run JSON"),
    )
    .expect("run document");
    fs::write(
        root.join(".skilltape/runs/run-a/events.jsonl"),
        b"{\"sequence\":0,\"status\":\"run_started\"}\n{\"sequence\":1,\"status\":\"run_succeeded\"}\n",
    )
    .expect("run events");
    fs::create_dir_all(root.join(".skilltape/receipts")).expect("receipts");
    fs::write(
        root.join(".skilltape/receipts/run-a.json"),
        serde_json::to_vec(&json!({
            "schema": "skilltape.dev/receipt/v1",
            "run_id": "run-a",
            "status": "succeeded"
        }))
        .expect("receipt JSON"),
    )
    .expect("receipt document");

    (temp, root)
}

async fn request(root: &Path, uri: &str, headers: &[(&str, &str)]) -> (StatusCode, String, Value) {
    let app = router(ConsoleReadModel::new(root).expect("read model"));
    let mut builder = Request::builder().uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let response = app
        .oneshot(builder.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024)
        .await
        .expect("response body");
    let body = String::from_utf8(bytes.to_vec()).expect("UTF-8 response");
    let document = serde_json::from_str(&body).unwrap_or(Value::Null);
    (status, format!("{content_type}\n{body}"), document)
}

#[tokio::test]
async fn workspace_and_tape_routes_return_bounded_read_models() {
    let (_temp, root) = fixture();
    let (status, _body, workspaces) = request(&root, "/api/v1/workspaces", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(workspaces["items"][0]["id"], "default");
    assert_eq!(workspaces["items"][0]["tape_count"], 1);

    let (status, _body, tapes) = request(
        &root,
        "/api/v1/workspaces/default/tapes?offset=0&limit=1",
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tapes["items"][0]["id"], "tape-a");
    assert_eq!(tapes["total"], 1);
    assert_eq!(tapes["next_offset"], Value::Null);
}

#[tokio::test]
async fn tape_listing_uses_the_storage_id_for_event_links_and_keeps_recorded_identity() {
    let (_temp, root) = fixture();
    fs::rename(
        root.join(".skilltape/tapes/tape-a"),
        root.join(".skilltape/tapes/friendly-name"),
    )
    .expect("rename storage directory without changing the recording");

    let (status, _body, tapes) = request(&root, "/api/v1/workspaces/default/tapes", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tapes["items"][0]["id"], "friendly-name");
    assert_eq!(tapes["items"][0]["recorded_id"], "tape-a");

    let (status, _body, events) = request(&root, "/api/v1/tapes/friendly-name/events", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(events["tape_id"], "friendly-name");
    assert_eq!(events["events"][0]["sequence"], 0);
    let saved: Value = serde_json::from_slice(
        &fs::read(root.join(".skilltape/tapes/friendly-name/manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(saved["id"], "tape-a");
}

#[tokio::test]
async fn tape_listing_rejects_an_oversized_manifest_before_reading_it() {
    let (_temp, root) = fixture();
    let manifest = root.join(".skilltape/tapes/tape-a/manifest.json");
    let mut document: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    document["workspace_root"] = Value::String("x".repeat(65 * 1024));
    fs::write(&manifest, serde_json::to_vec(&document).unwrap()).unwrap();

    let (status, _body, response) = request(&root, "/api/v1/workspaces/default/tapes", &[]).await;
    assert_ne!(status, StatusCode::OK);
    assert_eq!(response["error"]["code"], "invalid_document");
}

#[tokio::test]
async fn tape_events_preserve_redaction_and_support_pagination() {
    let (_temp, root) = fixture();
    let (status, _body, events) = request(&root, "/api/v1/tapes/tape-a/events?limit=1", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(events["events"][0]["redaction"], "redacted");
    assert_eq!(events["events"][0]["payload"]["output"], "redacted-value");
    assert_eq!(events["next_offset"], Value::Null);
}

#[test]
fn tape_pagination_rejects_a_source_over_the_read_budget() {
    let (_temp, root) = fixture();
    let path = root.join(".skilltape/tapes/tape-a/events.jsonl");
    let original = fs::read_to_string(&path).expect("synthetic event");
    // Legal JSON whitespace makes the stored line oversized without changing the event.
    fs::write(
        &path,
        format!("{}{}\n", original.trim_end(), " ".repeat(16 * 1024 * 1024)),
    )
    .expect("oversized synthetic event");
    let model = ConsoleReadModel::new(&root).expect("read model");
    assert!(model.tape_events("tape-a", 0, 1).is_err());
}

#[test]
fn tape_pagination_still_validates_events_outside_the_returned_page() {
    let (_temp, root) = fixture();
    let directory = root.join(".skilltape/tapes/tape-a");
    let first = fs::read_to_string(directory.join("events.jsonl")).expect("synthetic event");
    let mut second: Value = serde_json::from_str(&first).expect("event JSON");
    second["sequence"] = json!(99);
    fs::write(directory.join("events.jsonl"), format!("{first}{second}\n")).expect("events");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(directory.join("manifest.json")).unwrap()).unwrap();
    manifest["event_count"] = json!(2);
    fs::write(directory.join("manifest.json"), manifest.to_string()).expect("manifest");
    let model = ConsoleReadModel::new(&root).expect("read model");
    assert!(model.tape_events("tape-a", 0, 1).is_err());
}

#[tokio::test]
async fn skill_diff_exposes_hashes_and_lint_without_absolute_paths() {
    let (_temp, root) = fixture();
    let (status, body, diff) = request(&root, "/api/v1/skills/demo/diff", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(diff["package_path"], "skills/demo");
    assert_eq!(diff["policy_plan"]["execution"], "not-run");
    assert_eq!(diff["policy_plan"]["allowed"], true);
    assert!(diff["policy_plan"]["checks"].as_array().unwrap().len() <= 1000);
    assert_eq!(diff["lint"]["errors"].as_array().map(Vec::len), Some(0));
    assert_eq!(diff["files"].as_array().map(Vec::len), Some(6));
    assert!(!body.contains(root.to_string_lossy().as_ref()));
}

#[tokio::test]
async fn saved_document_routes_enforce_the_read_budget_without_changing_files() {
    let (_temp, root) = fixture();
    for (relative, uri) in [
        (".skilltape/runs/run-a/run.json", "/api/v1/runs/run-a"),
        (".skilltape/receipts/run-a.json", "/api/v1/receipts/run-a"),
    ] {
        let path = root.join(relative);
        let original = fs::read(&path).unwrap();
        // JSON whitespace exercises the input budget without a huge response body.
        let mut bytes = original.clone();
        bytes.resize(8 * 1024 * 1024, b' ');
        fs::write(&path, &bytes).unwrap();
        let (status, _, _) = request(&root, uri, &[]).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "document exactly at the limit: {uri}"
        );

        bytes.push(b' ');
        fs::write(&path, &bytes).unwrap();
        let (status, body, response) = request(&root, uri, &[]).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "oversized document: {uri}"
        );
        assert_eq!(response["error"]["code"], "invalid_document");
        assert!(!body.contains(root.to_string_lossy().as_ref()));
        assert_eq!(fs::read(&path).unwrap(), bytes);

        fs::write(&path, &original).unwrap();
        assert_eq!(request(&root, uri, &[]).await.0, StatusCode::OK);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn saved_document_routes_reject_symlinks_and_directories() {
    let (_temp, root) = fixture();
    let outside = root.join("outside.json");
    fs::write(&outside, b"{}").unwrap();
    for (relative, uri) in [
        (".skilltape/runs/run-a/run.json", "/api/v1/runs/run-a"),
        (".skilltape/receipts/run-a.json", "/api/v1/receipts/run-a"),
    ] {
        let path = root.join(relative);
        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&outside, &path).unwrap();
        assert_eq!(request(&root, uri, &[]).await.0, StatusCode::FORBIDDEN);
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert_eq!(request(&root, uri, &[]).await.0, StatusCode::FORBIDDEN);
    }
    assert_eq!(fs::read(outside).unwrap(), b"{}");
}

#[tokio::test]
async fn run_and_receipt_routes_return_stored_documents() {
    let (_temp, root) = fixture();
    let (status, _body, run) = request(&root, "/api/v1/runs/run-a", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(run["document"]["status"], "succeeded");

    let (status, _body, receipt) = request(&root, "/api/v1/receipts/run-a", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(receipt["document"]["schema"], "skilltape.dev/receipt/v1");
}

#[tokio::test]
async fn batch_review_reads_only_bounded_case_metadata_and_rejects_tampering() {
    use sha2::{Digest, Sha256};
    let (_temp, root) = fixture();
    let plan = serde_json::to_vec(&json!({"schema":"skilltape.dev/batch-plan/v1","skill":"private-source-skill","cases":[{"id":"north","input":"private-input-folder"}]})).unwrap();
    fs::write(root.join(".skilltape/batch-plan.json"), &plan).unwrap();
    let digest: String = Sha256::digest(&plan)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let mut state = json!({"schema":"skilltape.dev/batch-state/v1","plan_sha256":digest,"package_sha256":"a".repeat(64),"revision":1,"cases":[{"id":"north","input_sha256":"b".repeat(64),"generation":0,"attempts":[]}]});
    fs::write(
        root.join(".skilltape/batch.json"),
        serde_json::to_vec(&state).unwrap(),
    )
    .unwrap();
    let (status, body, report) = request(&root, "/api/v1/batch", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(report["status"], "pending");
    assert_eq!(report["cases"][0]["id"], "north");
    assert_eq!(report["max_cases"], 16);
    for private in [
        "private-source-skill",
        "private-input-folder",
        root.to_str().unwrap(),
    ] {
        assert!(!body.contains(private));
    }
    state["cases"][0]["id"] = json!("../outside");
    fs::write(
        root.join(".skilltape/batch.json"),
        serde_json::to_vec(&state).unwrap(),
    )
    .unwrap();
    assert_eq!(
        request(&root, "/api/v1/batch", &[]).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn run_events_are_replayable_sse_with_resume_and_terminal_event() {
    let (_temp, root) = fixture();
    let (status, body, _) = request(&root, "/api/v1/runs/run-a/events", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.starts_with("text/event-stream"));
    assert!(body.contains("id: 0"));
    assert!(body.contains("event: run"));
    assert!(body.contains("event: end"));
    assert!(body.contains("\"status\":\"complete\""));

    let (status, body, _) = request(
        &root,
        "/api/v1/runs/run-a/events",
        &[("Last-Event-ID", "0")],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.contains("id: 0\n"));
    assert!(body.contains("id: 1"));
    assert!(body.contains("id: 2"));
}

#[tokio::test]
async fn unsafe_ids_and_invalid_pagination_return_structured_errors() {
    let (_temp, root) = fixture();
    for uri in [
        "/api/v1/tapes/%2E%2E%2Foutside/events",
        "/api/v1/tapes/%5Coutside/events",
        "/api/v1/tapes/C%3Aoutside/events",
    ] {
        let (status, _body, error) = request(&root, uri, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(error["schema"], "skilltape.dev/api-error/v1");
        assert_eq!(error["error"]["code"], "unsafe_id");
    }

    let (status, _body, error) =
        request(&root, "/api/v1/workspaces/default/tapes?limit=101", &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["error"]["code"], "invalid_pagination");
}

#[cfg(unix)]
#[tokio::test]
async fn symlinked_storage_resource_is_forbidden() {
    let (_temp, root) = fixture();
    let outside = root.join("outside");
    fs::create_dir(&outside).expect("outside");
    std::os::unix::fs::symlink(&outside, root.join(".skilltape/tapes/linked"))
        .expect("tape symlink");

    let (status, _body, error) = request(&root, "/api/v1/tapes/linked/events", &[]).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error["error"]["code"], "unsafe_path");
}

#[cfg(unix)]
#[tokio::test]
async fn collection_endpoints_ignore_symlinked_storage_entries() {
    use std::os::unix::fs::symlink;

    let (_temp, root) = fixture();
    let outside_directory = root.join("outside-directory");
    let outside_receipt = root.join("outside-receipt.json");
    fs::create_dir(&outside_directory).expect("outside directory");
    fs::write(&outside_receipt, b"{}").expect("outside receipt");
    symlink(&outside_directory, root.join(".skilltape/tapes/linked")).expect("tape symlink");
    symlink(
        &outside_receipt,
        root.join(".skilltape/receipts/linked.json"),
    )
    .expect("receipt symlink");

    let (status, _body, workspaces) = request(&root, "/api/v1/workspaces", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(workspaces["items"][0]["tape_count"], 1);
    assert_eq!(workspaces["items"][0]["receipt_count"], 1);

    let (status, _body, tapes) = request(&root, "/api/v1/workspaces/default/tapes", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tapes["items"].as_array().expect("tape items").len(), 1);
    assert_eq!(tapes["items"][0]["id"], "tape-a");
}

#[tokio::test]
async fn static_console_assets_are_read_only_and_path_bounded() {
    let (_temp, root) = fixture();
    let ui = root.join("ui");
    fs::create_dir(&ui).expect("ui directory");
    fs::write(ui.join("index.html"), "<main>SkillTape Console</main>").expect("index");
    fs::write(ui.join("app.js"), "console.log('ok')").expect("asset");

    let app = router_with_static(
        ConsoleReadModel::new(&root).expect("read model"),
        Some(ui.canonicalize().expect("canonical UI root")),
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("index request"),
        )
        .await
        .expect("index response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(
        to_bytes(response.into_body(), 1024)
            .await
            .expect("index body"),
        "<main>SkillTape Console</main>"
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/app.js")
                .body(Body::empty())
                .expect("asset request"),
        )
        .await
        .expect("asset response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), 1024)
            .await
            .expect("asset body"),
        "console.log('ok')"
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/../outside")
                .body(Body::empty())
                .expect("escape request"),
        )
        .await
        .expect("escape response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn receipt_context_is_optional_bound_bounded_and_rejects_untrusted_fields() {
    use sha2::{Digest, Sha256};
    let (_temp, root) = fixture();
    let receipt = serde_json::to_vec(&json!({"schema":"skilltape.dev/receipt/v1", "run_id":"a".repeat(64),
        "skill_hash":"b".repeat(64), "status":"succeeded", "steps":[], "assertions":[], "policy_decisions":[]})).unwrap();
    let folder = root.join(".skilltape/receipts");
    fs::write(folder.join("run-a.json"), &receipt).unwrap();
    let uri = "/api/v1/receipts/run-a/context";
    assert_eq!(request(&root, uri, &[]).await.2["status"], "unrecorded");
    let hash: String = Sha256::digest(&receipt)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect();
    let mut context = json!({"schema":"skilltape.dev/run-context/v1", "receipt_sha256":hash,
        "run_id":"a".repeat(64), "skill_hash":"b".repeat(64), "input_set_sha256":"c".repeat(64),
        "input_observation":"bounded-before-run", "executor":"skilltape-runner/sequential-local-v1", "executor_version":"0.1.0",
        "os":"macos", "arch":"aarch64", "max_processes":1, "step_timeout_ms":1000, "max_output_bytes":65536});
    let sidecar = folder.join("run-a.json.context");
    fs::write(&sidecar, serde_json::to_vec(&context).unwrap()).unwrap();
    assert_eq!(request(&root, uri, &[]).await.2["status"], "recorded");
    context["receipt_sha256"] = json!("0".repeat(64));
    fs::write(&sidecar, serde_json::to_vec(&context).unwrap()).unwrap();
    assert_eq!(request(&root, uri, &[]).await.2["status"], "invalid");
    context["private_raw_input"] = json!("SYNTHETIC_MUST_NOT_BE_RETURNED");
    fs::write(&sidecar, serde_json::to_vec(&context).unwrap()).unwrap();
    let (_, body, document) = request(&root, uri, &[]).await;
    assert_eq!(document["context"], Value::Null);
    assert!(!body.contains("SYNTHETIC_MUST_NOT_BE_RETURNED"));
    fs::write(&sidecar, vec![b'a'; 4097]).unwrap();
    assert_eq!(request(&root, uri, &[]).await.2["status"], "invalid");
    assert_eq!(fs::read(folder.join("run-a.json")).unwrap(), receipt);
}

#[tokio::test]
async fn tape_coverage_reports_the_whole_capture_without_payloads() {
    let (_temp, root) = fixture();
    let store = TapeStore::create(
        root.join(".skilltape/tapes/coverage"),
        TapeManifest {
            schema: TAPE_SCHEMA_V1.to_owned(),
            id: "coverage".to_owned(),
            started_at_ms: 100,
            finished_at_ms: None,
            platform: "synthetic".to_owned(),
            workspace_root: "private-workspace-sentinel".to_owned(),
            event_count: 0,
        },
    )
    .unwrap();
    for sequence in 0..130 {
        let warning = sequence % 17 == 0;
        store
            .append(&TapeEvent {
                sequence,
                occurred_at_ms: 100 + sequence,
                kind: if warning {
                    TapeEventKind::CaptureWarning
                } else {
                    TapeEventKind::TerminalCommand
                },
                source: if warning {
                    EventSource::System
                } else {
                    EventSource::Shell
                },
                payload: json!({"secret":"SYNTHETIC_COVERAGE_PAYLOAD_SENTINEL"}),
                redaction: match sequence % 3 {
                    0 => RedactionState::Unredacted,
                    1 => RedactionState::PartiallyRedacted,
                    _ => RedactionState::Redacted,
                },
            })
            .unwrap();
    }
    store.finish(300).unwrap();
    let (status, body, value) = request(&root, "/api/v1/tapes/coverage/coverage", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["schema"], "skilltape.dev/tape-coverage/v1");
    assert_eq!(value["total_events"], 130);
    assert_eq!(value["state"], "complete");
    assert_eq!(value["redaction"]["redacted"], 43);
    assert_eq!(value["redaction"]["partially_redacted"], 43);
    assert_eq!(value["redaction"]["unredacted"], 44);
    assert_eq!(value["capture_warnings"], 8);
    assert_eq!(value["by_source"]["system"], 8);
    assert_eq!(value["samples"].as_array().unwrap().len(), 32);
    assert_eq!(value["samples_truncated"], true);
    assert!(!body.contains("SYNTHETIC_COVERAGE_PAYLOAD_SENTINEL"));
    assert!(!body.contains("private-workspace-sentinel"));
    assert!(!body.contains("\"payload\""));
}

#[tokio::test]
async fn tape_coverage_handles_empty_open_and_invalid_captures() {
    let (_temp, root) = fixture();
    TapeStore::create(
        root.join(".skilltape/tapes/empty"),
        TapeManifest {
            schema: TAPE_SCHEMA_V1.to_owned(),
            id: "empty".to_owned(),
            started_at_ms: 1,
            finished_at_ms: None,
            platform: "test".to_owned(),
            workspace_root: "relative".to_owned(),
            event_count: 0,
        },
    )
    .unwrap();
    let (status, _, value) = request(&root, "/api/v1/tapes/empty/coverage", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["total_events"], 0);
    assert_eq!(value["state"], "open");
    assert_eq!(value["first_event_at_ms"], Value::Null);
    fs::write(
        root.join(".skilltape/tapes/tape-a/events.jsonl"),
        b"{invalid json}\n",
    )
    .unwrap();
    let (status, _, value) = request(&root, "/api/v1/tapes/tape-a/coverage", &[]).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(value["error"]["code"], "invalid_document");
}

#[tokio::test]
async fn tape_coverage_keeps_storage_budget_and_identifier_guards() {
    let (_temp, root) = fixture();
    fs::OpenOptions::new()
        .write(true)
        .open(root.join(".skilltape/tapes/tape-a/events.jsonl"))
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    let (status, _, value) = request(&root, "/api/v1/tapes/tape-a/coverage", &[]).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(value["error"]["code"], "invalid_document");
    let (status, _, _) = request(&root, "/api/v1/tapes/..%2Foutside/coverage", &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn tape_reads_keep_safe_dotted_identifiers_and_reject_encoded_escape_paths() {
    let (_temp, root) = fixture();
    fs::rename(
        root.join(".skilltape/tapes/tape-a"),
        root.join(".skilltape/tapes/tape..saved"),
    )
    .unwrap();
    for resource in ["coverage", "events"] {
        let (status, _, _) =
            request(&root, &format!("/api/v1/tapes/tape..saved/{resource}"), &[]).await;
        assert_eq!(status, StatusCode::OK);
        for id in ["..%2Foutside", "..%5Coutside", "%2Fabsolute", "C%3Aother"] {
            let (status, _, _) =
                request(&root, &format!("/api/v1/tapes/{id}/{resource}"), &[]).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }
    }
}

#[tokio::test]
async fn tape_reads_reject_a_manifest_directory_as_an_invalid_document() {
    let (_temp, root) = fixture();
    let manifest = root.join(".skilltape/tapes/tape-a/manifest.json");
    fs::remove_file(&manifest).unwrap();
    fs::create_dir(&manifest).unwrap();
    for resource in ["coverage", "events"] {
        let (status, _, value) =
            request(&root, &format!("/api/v1/tapes/tape-a/{resource}"), &[]).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(value["error"]["code"], "invalid_document");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn coverage_rejects_linked_tape_roots_and_members() {
    use std::os::unix::fs::symlink;
    let (_temp, root) = fixture();
    let original = root.join(".skilltape/tapes/tape-a");
    symlink(&original, root.join(".skilltape/tapes/linked")).unwrap();
    let (status, _, _) = request(&root, "/api/v1/tapes/linked/coverage", &[]).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let events = original.join("events.jsonl");
    fs::rename(&events, original.join("retained-events.jsonl")).unwrap();
    symlink(original.join("retained-events.jsonl"), &events).unwrap();
    let (status, _, _) = request(&root, "/api/v1/tapes/tape-a/coverage", &[]).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
