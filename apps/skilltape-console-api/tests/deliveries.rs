#![cfg(unix)]

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use skilltape_console_api::{router, ConsoleReadModel};
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use tower::ServiceExt;

fn sha(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn bundle(root: &Path, name: &str) {
    let target = root.join(name);
    fs::create_dir_all(target.join("artifacts")).unwrap();
    let content = b"bounded synthetic delivery\n";
    fs::write(target.join("artifacts/report.txt"), content).unwrap();
    let receipt = serde_json::to_vec(&json!({"schema":"skilltape.dev/receipt/v1",
        "run_id":"a".repeat(64),"skill_hash":"b".repeat(64),"status":"succeeded",
        "steps":[],"assertions":[],"policy_decisions":[]}))
    .unwrap();
    fs::write(target.join("receipt.json"), &receipt).unwrap();
    let digest = sha(content);
    let mut set = Sha256::new();
    set.update(b"report.txt\0");
    set.update((content.len() as u64).to_be_bytes());
    set.update(digest.as_bytes());
    let set_hash: String = set
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    fs::write(target.join("delivery.json"), serde_json::to_vec(&json!({
        "schema":"skilltape.dev/delivery/v1","run_id":"a".repeat(64),"skill_hash":"b".repeat(64),
        "receipt_sha256":sha(&receipt),"artifact_set_sha256":set_hash,
        "files":[{"path":"report.txt","bytes":content.len(),"sha256":digest}]
    })).unwrap()).unwrap();
}

async fn request(root: &Path, uri: &str) -> (StatusCode, Value) {
    let response = router(ConsoleReadModel::new(root).unwrap())
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 2 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body).unwrap()
        },
    )
}

#[tokio::test]
async fn discovers_real_delivery_layout_without_legacy_run_registry() {
    let root = TempDir::new().unwrap();
    bundle(root.path(), "saved-b");
    bundle(root.path(), "saved-a");
    fs::create_dir(root.path().join("ordinary-input")).unwrap();
    let (status, page) =
        request(root.path(), "/api/v1/workspaces/default/deliveries?limit=1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["total"], 2);
    assert_eq!(page["next_offset"], 1);
    assert_eq!(page["items"][0]["id"], "saved-a");
    assert_eq!(page["items"][0]["reported_status"], "succeeded");
    assert_eq!(page["items"][0]["declared_files"], 1);
    let (_, second) = request(
        root.path(),
        "/api/v1/workspaces/default/deliveries?limit=1&offset=1",
    )
    .await;
    assert_eq!(second["items"][0]["id"], "saved-b");
    assert!(!root.path().join(".skilltape").exists());
}

#[tokio::test]
async fn physically_checks_files_without_claiming_execution_or_requirements() {
    let root = TempDir::new().unwrap();
    bundle(root.path(), "saved");
    let (status, report) = request(root.path(), "/api/v1/deliveries/saved").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(report["status"], "passed");
    assert_eq!(report["receipt_binding"], "verified");
    assert_eq!(report["reported_status"], "succeeded");
    assert_eq!(report["checked_files"], 1);
    assert_eq!(report["requirement_validation"], "not-run");
    assert_eq!(report["provenance"], "not-authenticated");
    assert_eq!(report["files"][0]["path"], "report.txt");
    assert!(!report.to_string().contains("bounded synthetic delivery"));
    assert!(!report.to_string().contains(root.path().to_str().unwrap()));
    assert_eq!(
        fs::read(root.path().join("saved/artifacts/report.txt")).unwrap(),
        b"bounded synthetic delivery\n"
    );
}

#[tokio::test]
async fn rejects_mutation_missing_members_and_undeclared_private_files() {
    let root = TempDir::new().unwrap();
    bundle(root.path(), "saved");
    fs::write(root.path().join("saved/artifacts/report.txt"), b"changed").unwrap();
    let (_, report) = request(root.path(), "/api/v1/deliveries/saved").await;
    assert_eq!(report["status"], "failed");
    fs::remove_file(root.path().join("saved/artifacts/report.txt")).unwrap();
    let (_, missing) = request(root.path(), "/api/v1/deliveries/saved").await;
    assert_eq!(missing["status"], "failed");
    fs::write(
        root.path().join("saved/artifacts/.env"),
        b"NEVER_EXPOSE_PRIVATE_SENTINEL",
    )
    .unwrap();
    let (_, private) = request(root.path(), "/api/v1/deliveries/saved").await;
    assert_eq!(private["status"], "failed");
    assert!(!private
        .to_string()
        .contains("NEVER_EXPOSE_PRIVATE_SENTINEL"));
    assert_eq!(private["checked_files"], 0);
}

#[tokio::test]
async fn refuses_symlinks_traversal_and_metadata_capacity() {
    use std::os::unix::fs::symlink;
    let root = TempDir::new().unwrap();
    bundle(root.path(), "saved");
    symlink(root.path().join("saved"), root.path().join("alias")).unwrap();
    let (status, _) = request(root.path(), "/api/v1/deliveries/alias").await;
    assert_ne!(status, StatusCode::OK);
    let (status, _) = request(root.path(), "/api/v1/deliveries/%2e%2e").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    fs::write(
        root.path().join("saved/receipt.json"),
        vec![b'x'; 1024 * 1024 + 1],
    )
    .unwrap();
    let (_, report) = request(root.path(), "/api/v1/deliveries/saved").await;
    assert_eq!(report["status"], "failed");
    assert_eq!(report["checked_files"], 0);
    let (_, list) = request(root.path(), "/api/v1/workspaces/default/deliveries").await;
    assert_eq!(list["items"][0]["metadata_valid"], false);
}

#[tokio::test]
async fn bounds_discovery_and_preserves_empty_or_invalid_page_states() {
    let root = TempDir::new().unwrap();
    let (status, page) = request(root.path(), "/api/v1/workspaces/default/deliveries").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["total"], 0);
    let (status, _) = request(
        root.path(),
        "/api/v1/workspaces/default/deliveries?limit=101",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    for i in 0..1001 {
        fs::write(root.path().join(format!("entry-{i}")), b"").unwrap();
    }
    let (status, _) = request(root.path(), "/api/v1/workspaces/default/deliveries").await;
    assert_ne!(status, StatusCode::OK);
}

#[tokio::test]
async fn rejects_duplicate_fields_and_unsafe_declarations_before_payload_reads() {
    let root = TempDir::new().unwrap();
    bundle(root.path(), "saved");
    let receipt_path = root.path().join("saved/receipt.json");
    let original = fs::read_to_string(&receipt_path).unwrap();
    let repeated = original.replacen(
        "\"status\":\"succeeded\"",
        "\"status\":\"run_failed\",\"status\":\"succeeded\"",
        1,
    );
    fs::write(&receipt_path, &repeated).unwrap();
    let manifest_path = root.path().join("saved/delivery.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["receipt_sha256"] = sha(repeated.as_bytes()).into();
    fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let (_, invalid) = request(root.path(), "/api/v1/deliveries/saved").await;
    assert_eq!(invalid["status"], "failed");
    assert_eq!(invalid["checked_files"], 0);
    fs::write(&receipt_path, &original).unwrap();
    for path in ["../escape.txt", ".env", "secrets/credentials.json"] {
        manifest["receipt_sha256"] = sha(original.as_bytes()).into();
        manifest["files"][0]["path"] = path.into();
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let (_, invalid) = request(root.path(), "/api/v1/deliveries/saved").await;
        assert_eq!(invalid["status"], "failed");
        assert_eq!(invalid["checked_files"], 0);
    }
}

#[tokio::test]
async fn refuses_non_loopback_bind_before_starting_a_service() {
    let root = TempDir::new().unwrap();
    let result = skilltape_console_api::serve(root.path(), "0.0.0.0:0".parse().unwrap()).await;
    assert!(matches!(
        result,
        Err(skilltape_console_api::ServeError::NonLoopback)
    ));
}
