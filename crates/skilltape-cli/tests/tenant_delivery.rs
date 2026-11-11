#![cfg(any(target_os = "linux", target_os = "macos"))]

use assert_cmd::Command;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

fn package() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/verified-tenant-delivery")
}

fn verify(input: &std::path::Path, delivery: &std::path::Path) -> assert_cmd::assert::Assert {
    Command::cargo_bin("skilltape")
        .expect("binary")
        .arg("verify")
        .arg(package())
        .arg("--input")
        .arg(input)
        .arg("--delivery-dir")
        .arg(delivery)
        .arg("--json")
        .assert()
}

#[test]
fn tenant_local_ids_and_split_shipments_survive_a_real_delivery() {
    Command::cargo_bin("skilltape")
        .expect("binary")
        .arg("lint")
        .arg(package())
        .args(["--strict", "--json"])
        .assert()
        .success();
    let temp = tempfile::tempdir().expect("job");
    for wrong in [false, true] {
        let input = temp
            .path()
            .join(if wrong { "fault-input" } else { "good-input" });
        fs::create_dir(&input).expect("input");
        fs::copy(
            package().join("fixtures/input/orders.csv"),
            input.join("orders.csv"),
        )
        .expect("orders");
        fs::copy(
            package().join(if wrong {
                "fixtures/faults/shifted-shipments.csv"
            } else {
                "fixtures/input/shipments.csv"
            }),
            input.join("shipments.csv"),
        )
        .expect("shipments");
        let delivery = temp.path().join(if wrong {
            "fault-delivery"
        } else {
            "good-delivery"
        });
        let assertion = verify(&input, &delivery).success();
        let stdout = &assertion.get_output().stdout;
        let receipt: Value = serde_json::from_slice(stdout).expect("Receipt");
        assert_eq!(receipt["status"], "succeeded");
        assert!(delivery.join("receipt.json").is_file());
        assert!(delivery.join("delivery.json").is_file());
        assert_eq!(
            fs::read_to_string(delivery.join("artifacts/expected.csv")).expect("expected"),
            "tenant_id,order_id,amount_cents\nORG_A,ORDER_1,1000\nORG_B,ORDER_1,2000\n"
        );
        let allocations =
            fs::read_to_string(delivery.join("artifacts/allocations.csv")).expect("allocations");
        assert_eq!(
            allocations,
            if wrong {
                "tenant_id,order_id,amount_cents\nORG_A,ORDER_1,1100\nORG_B,ORDER_1,1900\n"
            } else {
                "tenant_id,order_id,amount_cents\nORG_A,ORDER_1,1000\nORG_B,ORDER_1,2000\n"
            }
        );
        let summary: Value = serde_json::from_slice(
            &fs::read(delivery.join("artifacts/summary.json")).expect("summary"),
        )
        .expect("JSON");
        assert_eq!(
            summary,
            serde_json::json!({"expected_rows":2,"allocation_rows":2,"total_cents":3000})
        );
        assert!(!String::from_utf8_lossy(stdout).contains("ORG_A"));
        assert!(!String::from_utf8_lossy(stdout).contains("ORDER_1"));
    }
}

#[test]
fn invalid_or_exhausted_inputs_never_publish_a_tenant_delivery() {
    let temp = tempfile::tempdir().expect("job");
    for (name, orders, shipments) in [
        (
            "duplicate",
            "tenant_id,order_id,amount_cents\nA,I,1\nA,I,2\n".to_owned(),
            "tenant_id,order_id,amount_cents\nA,I,3\n".to_owned(),
        ),
        (
            "negative",
            "tenant_id,order_id,amount_cents\nA,I,1\n".to_owned(),
            "tenant_id,order_id,amount_cents\nA,I,-1\n".to_owned(),
        ),
        (
            "identity",
            "tenant_id,order_id,amount_cents\nA,I,1\n".to_owned(),
            "tenant_id,order_id,amount_cents\n../OUTSIDE,I,1\n".to_owned(),
        ),
        (
            "unsafe",
            "tenant_id,order_id,amount_cents\nA,I,9007199254740992\n".to_owned(),
            "tenant_id,order_id,amount_cents\nA,I,1\n".to_owned(),
        ),
        (
            "rows",
            "tenant_id,order_id,amount_cents\nA,I,1\n".to_owned(),
            "tenant_id,order_id,amount_cents\n".to_owned() + &"A,I,1\n".repeat(1000),
        ),
        (
            "bytes",
            "tenant_id,order_id,amount_cents\nA,I,1\n".to_owned(),
            "x".repeat(1024 * 1024),
        ),
    ] {
        let input = temp.path().join(format!("input-{name}"));
        fs::create_dir(&input).expect("input");
        fs::write(input.join("orders.csv"), orders).expect("orders");
        fs::write(input.join("shipments.csv"), shipments).expect("shipments");
        let delivery = temp.path().join(format!("delivery-{name}"));
        let assertion = verify(&input, &delivery).code(3);
        let receipt: Value =
            serde_json::from_slice(&assertion.get_output().stdout).expect("failed Receipt");
        assert_eq!(receipt["status"], "run_failed");
        assert_eq!(receipt["steps"][0]["exit_code"], 1);
        let failure = b"tenant-delivery input or output is invalid\n";
        assert_eq!(
            receipt["steps"][0]["stderr_bytes"],
            failure.len(),
            "case {name}"
        );
        let failure_hash: String = Sha256::digest(failure)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(receipt["steps"][0]["stderr_sha256"], failure_hash);
        assert!(!delivery.exists());
    }
}

#[test]
fn producer_refuses_existing_outputs_without_replacing_them() {
    let temp = tempfile::tempdir().expect("outputs");
    let sentinel = temp.path().join("expected.csv");
    fs::write(&sentinel, "KEEP").expect("sentinel");
    let result = std::process::Command::new("/bin/bash")
        .current_dir(package())
        .args([
            "scripts/run_tenants.sh",
            "--orders",
            "fixtures/input/orders.csv",
            "--shipments",
            "fixtures/input/shipments.csv",
            "--output-dir",
        ])
        .arg(temp.path())
        .output()
        .expect("local synthetic script");
    assert!(!result.status.success());
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        "tenant-delivery input or output is invalid\n"
    );
    assert_eq!(fs::read_to_string(sentinel).expect("sentinel"), "KEEP");
    assert!(!temp.path().join("allocations.csv").exists());
}
