use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use skilltape_console_api::{router, ConsoleReadModel};
use std::{fs, path::Path};
use tempfile::TempDir;
use tower::ServiceExt;

fn receipt(run: &str, status: &str) -> Value {
    json!({"schema":"skilltape.dev/receipt/v1","run_id":run.repeat(64),"skill_hash":"c".repeat(64),
        "status":status,"steps":[],"assertions":[],"policy_decisions":[]})
}
fn assertion(target: &str, passed: bool) -> Value {
    json!({"kind":"file_hash","target":target,"passed":passed,"reason":"PRIVATE_REASON_SENTINEL"})
}
fn step(id: &str, status: &str, exit: i32) -> Value {
    json!({"step_id":id,"status":status,"exit_code":exit,"stdout_sha256":"d".repeat(64),"stdout_bytes":1,
      "stdout_truncated":false,"stderr_sha256":"e".repeat(64),"stderr_bytes":1,"stderr_truncated":false})
}
fn write(root: &Path, id: &str, value: &Value) {
    fs::write(
        root.join(format!(".skilltape/receipts/{id}.json")),
        serde_json::to_vec(value).unwrap(),
    )
    .unwrap();
}
fn fixture() -> TempDir {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join(".skilltape/receipts")).unwrap();
    root
}

#[tokio::test]
async fn a_saved_receipt_named_compare_keeps_its_original_read_route() {
    let root = fixture();
    write(root.path(), "compare", &receipt("a", "succeeded"));
    let (status, data, _) = get(root.path(), "/api/v1/receipts/compare").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(data["id"], "compare");
}
async fn get(root: &Path, uri: &str) -> (StatusCode, Value, String) {
    let response = router(ConsoleReadModel::new(root).unwrap())
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        body,
    )
}
fn context(root: &Path, id: &str, input: &str) {
    let bytes = fs::read(root.join(format!(".skilltape/receipts/{id}.json"))).unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    let digest: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let data = json!({"schema":"skilltape.dev/run-context/v1","receipt_sha256":digest,"run_id":value["run_id"],
      "skill_hash":value["skill_hash"],"input_set_sha256":input.repeat(64),"input_observation":"bounded-before-run",
      "executor":"skilltape-runner/sequential-local-v1","executor_version":"0.1.0","os":"linux","arch":"x86_64",
      "max_processes":1,"step_timeout_ms":1000,"max_output_bytes":65536});
    fs::write(
        root.join(format!(".skilltape/receipts/{id}.json.context")),
        serde_json::to_vec(&data).unwrap(),
    )
    .unwrap();
}
#[tokio::test]
async fn compares_grouped_outcomes_without_losing_repeated_checks_or_private_reasons() {
    let root = fixture();
    let path = root.path();
    let mut before = receipt("a", "run_failed");
    let mut after = receipt("b", "assertion_failed");
    before["assertions"] = json!([
        assertion("output.txt", false),
        assertion("output.txt", false),
        assertion("old.txt", false),
        assertion("changed.txt", true)
    ]);
    after["assertions"] = json!([
        assertion("output.txt", false),
        assertion("output.txt", true),
        assertion("changed.txt", false),
        assertion("new.txt", false)
    ]);
    before["steps"] = json!([step("build", "failed", 1), step("copy", "succeeded", 0)]);
    after["steps"] = json!([step("build", "succeeded", 0), step("copy", "succeeded", 0)]);
    let policy = json!([{"step_id":"build","phase":"before","allowed":false,"code":"denied","reason":"PRIVATE_REASON_SENTINEL","risk":"high"}]);
    before["policy_decisions"] = policy.clone();
    after["policy_decisions"] = policy;
    write(path, "before", &before);
    write(path, "after", &after);
    let saved = fs::read(path.join(".skilltape/receipts/before.json")).unwrap();
    let (status, data, body) = get(
        path,
        "/api/v1/receipt-comparisons?before=before&after=after",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(data["comparability"], "unknown");
    assert_eq!(data["counts"]["new_non_success"], 2);
    assert_eq!(data["counts"]["no_longer_observed_non_success"], 3);
    assert_eq!(data["counts"]["continuing_non_success"], 2);
    assert_eq!(data["counts"]["total_groups"], 7);
    assert_eq!(data["execution"], "not-run");
    assert_eq!(data["export_approval"], "not-granted");
    assert!(!body.contains("PRIVATE_REASON_SENTINEL"));
    assert!(!body.contains("stdout_sha256"));
    assert_eq!(
        fs::read(path.join(".skilltape/receipts/before.json")).unwrap(),
        saved
    );
}
#[tokio::test]
async fn recorded_conditions_and_package_changes_cannot_be_called_equivalent() {
    let root = fixture();
    let path = root.path();
    write(path, "before", &receipt("a", "succeeded"));
    write(path, "after", &receipt("b", "succeeded"));
    context(path, "before", "d");
    context(path, "after", "d");
    assert_eq!(
        get(
            path,
            "/api/v1/receipt-comparisons?before=before&after=after"
        )
        .await
        .1["comparability"],
        "same-recorded-conditions"
    );
    context(path, "after", "e");
    let data = get(
        path,
        "/api/v1/receipt-comparisons?before=before&after=after",
    )
    .await
    .1;
    assert_eq!(data["comparability"], "incomparable");
    assert!(data["condition_findings"]
        .as_array()
        .unwrap()
        .contains(&json!("input_changed")));
    let mut changed = receipt("b", "succeeded");
    changed["skill_hash"] = json!("f".repeat(64));
    write(path, "after", &changed);
    let data = get(
        path,
        "/api/v1/receipt-comparisons?before=before&after=after",
    )
    .await
    .1;
    assert_eq!(data["comparability"], "incomparable");
    assert!(data["condition_findings"]
        .as_array()
        .unwrap()
        .contains(&json!("package_changed")));
}
#[tokio::test]
async fn comparison_bounds_details_and_rejects_unsafe_ids_invalid_or_oversized_rows() {
    let root = fixture();
    let path = root.path();
    let mut before = receipt("a", "assertion_failed");
    let mut after = receipt("b", "succeeded");
    before["assertions"] = json!((0..80)
        .map(|n| assertion(&format!("file-{n}.txt"), false))
        .collect::<Vec<_>>());
    write(path, "before", &before);
    write(path, "after", &after);
    let data = get(
        path,
        "/api/v1/receipt-comparisons?before=before&after=after",
    )
    .await
    .1;
    assert_eq!(data["change_total"], 80);
    assert_eq!(data["changes"].as_array().unwrap().len(), 64);
    assert_eq!(data["changes_truncated"], true);
    before["assertions"] = json!(vec![assertion("x", false); 1001]);
    write(path, "before", &before);
    assert_eq!(
        get(
            path,
            "/api/v1/receipt-comparisons?before=before&after=after"
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    after["assertions"] = json!([assertion("contradiction", false)]);
    write(path, "after", &after);
    write(path, "before", &receipt("a", "succeeded"));
    assert_eq!(
        get(
            path,
            "/api/v1/receipt-comparisons?before=before&after=after"
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    for uri in [
        "/api/v1/receipt-comparisons?before=before&after=before",
        "/api/v1/receipt-comparisons?before=..%2Foutside&after=after",
    ] {
        assert_eq!(get(path, uri).await.0, StatusCode::BAD_REQUEST);
    }
}
#[tokio::test]
async fn cancelled_is_a_real_v1_status_and_policy_denial_is_not_a_receipt_status() {
    let root = fixture();
    write(root.path(), "cancelled", &receipt("a", "cancelled"));
    let (status, data, _) = get(root.path(), "/api/v1/receipts?status=cancelled").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(data["total"], 1);
    assert_eq!(data["items"][0]["metadata_state"], "available");
    assert_eq!(
        get(root.path(), "/api/v1/receipts?status=policy_denied")
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
}
