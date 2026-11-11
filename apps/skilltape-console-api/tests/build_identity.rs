use axum::{
    body::{to_bytes, Body},
    http::Request,
};
use skilltape_console_api::{router, ConsoleReadModel};
use tower::ServiceExt;
#[tokio::test]
async fn identity_is_content_free_and_never_cacheable() {
    let directory = tempfile::tempdir().unwrap();
    let response = router(ConsoleReadModel::new(directory.path()).unwrap())
        .oneshot(
            Request::builder()
                .uri("/api/v1/build-info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let raw = to_bytes(response.into_body(), 16384).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(value["schema"], "skilltape.build-info/v1");
    assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}
