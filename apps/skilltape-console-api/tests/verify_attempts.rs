use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use serde_json::Value;
use skilltape_console_api::{router, ConsoleReadModel};
use skilltape_verify::attempt::{digest, FINISH_SCHEMA, START_SCHEMA};
use std::{fs, path::Path};
use tower::ServiceExt;

fn start(root: &Path, id: &str, time: u64) -> Vec<u8> {
    fs::create_dir_all(root.join(".skilltape/attempts")).unwrap();
    let raw =
        serde_json::to_vec(&json!({"schema":START_SCHEMA,"id":id,"started_at_ms":time})).unwrap();
    fs::write(
        root.join(format!(".skilltape/attempts/{id}.started.json")),
        &raw,
    )
    .unwrap();
    raw
}
async fn get(root: &Path, uri: &str) -> (StatusCode, Value) {
    let r = router(ConsoleReadModel::new(root).unwrap())
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = r.status();
    let body = to_bytes(r.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

#[tokio::test]
async fn empty_attempt_collection_is_readonly_without_changing_saved_receipts() {
    let root = tempfile::TempDir::new().unwrap();
    let response = router(ConsoleReadModel::new(root.path()).unwrap())
        .oneshot(
            Request::builder()
                .uri("/api/v1/verify-attempts?limit=12&offset=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 8192).await.unwrap();
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["total"], 0);
    assert_eq!(value["items"].as_array().unwrap().len(), 0);
    assert!(
        !root.path().join(".skilltape").exists(),
        "inspection must not initialize a journal"
    );
}

#[tokio::test]
async fn incomplete_attempts_stay_unconfirmed_and_completion_is_only_metadata_bound() {
    let root = tempfile::TempDir::new().unwrap();
    let id = "a".repeat(64);
    let raw = start(root.path(), &id, 1);
    let (status, body) = get(root.path(), &format!("/api/v1/verify-attempts/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["outcome"], "unconfirmed");
    assert_eq!(body["record_integrity"], "start-only");
    assert_eq!(body["read_only"], true);
    assert_eq!(body["saved_receipt_verified"], false);
    let finish = json!({"schema":FINISH_SCHEMA,"id":id,"start_sha256":digest(&raw),"finished_at_ms":2,"outcome":"receipt_produced",
  "receipt":{"receipt_sha256":"b".repeat(64),"run_id":"c".repeat(64),"skill_hash":"d".repeat(64),"input_set_sha256":"e".repeat(64),"status":"cancelled"}});
    let path = root
        .path()
        .join(format!(".skilltape/attempts/{id}.finished.json"));
    fs::write(&path, serde_json::to_vec(&finish).unwrap()).unwrap();
    let original = fs::read(&path).unwrap();
    let (status, body) = get(root.path(), "/api/v1/verify-attempts?limit=12&offset=0").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"][0]["outcome"], "receipt_produced");
    assert_eq!(body["items"][0]["receipt"]["status"], "cancelled");
    assert_eq!(body["items"][0]["provenance"], "not-authenticated");
    assert_eq!(fs::read(&path).unwrap(), original);
    fs::write(
        root.path()
            .join(format!(".skilltape/attempts/{id}.started.json")),
        b"{}",
    )
    .unwrap();
    assert_eq!(
        get(root.path(), &format!("/api/v1/verify-attempts/{id}"))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn forged_or_oversized_completion_and_unsafe_ids_never_show_a_receipt() {
    let root = tempfile::TempDir::new().unwrap();
    let id = "a".repeat(64);
    start(root.path(), &id, 1);
    let path = root
        .path()
        .join(format!(".skilltape/attempts/{id}.finished.json"));
    let wrong = json!({"schema":FINISH_SCHEMA,"id":id,"start_sha256":"b".repeat(64),"finished_at_ms":2,"outcome":"receipt_produced","receipt":null});
    fs::write(&path, serde_json::to_vec(&wrong).unwrap()).unwrap();
    assert_eq!(
        get(root.path(), &format!("/api/v1/verify-attempts/{id}"))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    fs::write(&path, vec![b'x'; 4097]).unwrap();
    assert_eq!(
        get(root.path(), &format!("/api/v1/verify-attempts/{id}"))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        get(root.path(), "/api/v1/verify-attempts/not-a-digest")
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get(root.path(), "/api/v1/verify-attempts?limit=0").await.0,
        StatusCode::BAD_REQUEST
    );
}

#[cfg(unix)]
#[tokio::test]
async fn attempt_links_and_discovery_overflow_are_rejected() {
    let root = tempfile::TempDir::new().unwrap();
    let outside = tempfile::TempDir::new().unwrap();
    let id = "a".repeat(64);
    start(outside.path(), &id, 1);
    fs::create_dir_all(root.path().join(".skilltape")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join(".skilltape/attempts"),
        root.path().join(".skilltape/attempts"),
    )
    .unwrap();
    assert_eq!(
        get(root.path(), "/api/v1/verify-attempts").await.0,
        StatusCode::FORBIDDEN
    );
    let full = tempfile::TempDir::new().unwrap();
    for n in 0..257 {
        start(full.path(), &format!("{n:064x}"), 1);
    }
    let (status, body) = get(full.path(), "/api/v1/verify-attempts").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "attempt_discovery_capacity");
}

#[cfg(unix)]
#[tokio::test]
async fn attempt_hardlinks_are_rejected_before_reading_their_bodies() {
    let root = tempfile::TempDir::new().unwrap();
    let outside = tempfile::TempDir::new().unwrap();
    let id = "a".repeat(64);
    start(root.path(), &id, 1);
    let file = root
        .path()
        .join(format!(".skilltape/attempts/{id}.started.json"));
    fs::hard_link(&file, outside.path().join("retained.json")).unwrap();
    assert_eq!(
        get(root.path(), &format!("/api/v1/verify-attempts/{id}"))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        fs::read(&file).unwrap(),
        fs::read(outside.path().join("retained.json")).unwrap()
    );
}
