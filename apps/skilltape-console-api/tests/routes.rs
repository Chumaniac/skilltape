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
async fn tape_events_preserve_redaction_and_support_pagination() {
    let (_temp, root) = fixture();
    let (status, _body, events) = request(&root, "/api/v1/tapes/tape-a/events?limit=1", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(events["events"][0]["redaction"], "redacted");
    assert_eq!(events["events"][0]["payload"]["output"], "redacted-value");
    assert_eq!(events["next_offset"], Value::Null);
}

#[tokio::test]
async fn skill_diff_exposes_hashes_and_lint_without_absolute_paths() {
    let (_temp, root) = fixture();
    let (status, body, diff) = request(&root, "/api/v1/skills/demo/diff", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(diff["package_path"], "skills/demo");
    assert_eq!(diff["lint"]["errors"].as_array().map(Vec::len), Some(0));
    assert_eq!(diff["files"].as_array().map(Vec::len), Some(6));
    assert!(!body.contains(root.to_string_lossy().as_ref()));
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
