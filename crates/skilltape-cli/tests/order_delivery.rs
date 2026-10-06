#![cfg(any(target_os = "linux", target_os = "macos"))]

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::Path;

#[test]
fn the_synthetic_order_script_runs_and_its_delivery_survives_the_cli() {
    let package = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/verified-order-delivery")
        .canonicalize()
        .expect("example");
    let temp = tempfile::tempdir().expect("job");
    let delivery = temp.path().join("delivery");
    Command::cargo_bin("skilltape")
        .expect("binary")
        .arg("lint")
        .arg(&package)
        .args(["--strict", "--json"])
        .assert()
        .success();
    let result = Command::cargo_bin("skilltape")
        .expect("binary")
        .arg("verify")
        .arg(&package)
        .arg("--input")
        .arg(package.join("fixtures/input"))
        .arg("--delivery-dir")
        .arg(&delivery)
        .arg("--json")
        .assert();
    #[cfg(target_os = "macos")]
    if !result.get_output().status.success() {
        // The public Receipt deliberately omits stderr. Diagnose only this fixed,
        // synthetic interpreter startup without reading user data or environment.
        use skilltape_runner::{ProcessAdapter, ProcessRequest, TokioProcessAdapter};
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("diagnostic runtime");
        let probe = runtime.block_on(TokioProcessAdapter.run(
            ProcessRequest {
                program: "/usr/bin/perl".into(),
                args: vec![
                    "-e".into(),
                    "use JSON::PP; use Encode; use Getopt::Long; print qq(perl-ready\\n);".into(),
                ],
                cwd: temp.path().to_path_buf(),
                timeout: std::time::Duration::from_secs(3),
                max_output_bytes: 1024,
            },
            tokio_util::sync::CancellationToken::new(),
        ));
        eprintln!("controlled interpreter startup: {probe:?}");
    }
    let stdout = result.success().get_output().stdout.clone();
    let receipt: Value = serde_json::from_slice(&stdout).expect("Receipt");
    assert_eq!(receipt["status"], "succeeded");
    assert_eq!(
        fs::read_to_string(delivery.join("artifacts/metrics.csv")).expect("actual CSV"),
        "day,orders,total_cents\n2026-01-01,2,2000\n2026-01-02,1,1600\n"
    );
    let summary: Value = serde_json::from_slice(
        &fs::read(delivery.join("artifacts/summary.json")).expect("actual JSON"),
    )
    .expect("summary");
    assert_eq!(summary["row_count"], 2);
    assert_eq!(summary["total_orders"], 3);
    assert_eq!(summary["total_cents"], 3600);
    assert!(delivery.join("receipt.json").is_file());
    assert!(delivery.join("delivery.json").is_file());
    assert!(!String::from_utf8_lossy(&stdout).contains("DEMO-001"));
}
