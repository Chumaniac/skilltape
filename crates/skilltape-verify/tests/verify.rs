use std::fs;
use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};
use skilltape_core::{create_skill_template, LoadedSkillPackage, SkillPackage};
use skilltape_runner::ResourceLimits;
use skilltape_verify::{verify_run, Assertion, ReceiptStatus, VerifyError, VerifyRequest};
use tempfile::{tempdir, TempDir};

struct PackageFixture {
    _root: TempDir,
    package: LoadedSkillPackage,
}

fn permissions(read: &[&str], write: &[&str], executables: &[&str]) -> Value {
    json!({
        "schema": "skilltape.dev/permissions/v1",
        "filesystem": {"read": read, "write": write},
        "process": {
            "executables": executables,
            "max_processes": 1,
            "default_timeout_ms": 1000
        },
        "network": {"enabled": false, "allow_hosts": []},
        "secrets": {"read_environment": false}
    })
}

fn package(steps: Vec<Value>, permission_document: Value) -> PackageFixture {
    let root = tempdir().expect("package root");
    let package_root = root.path().join("package");
    create_skill_template(&package_root, "verify-test").expect("template");
    fs::write(
        package_root.join("workflow.yaml"),
        serde_json::to_vec(&json!({
            "schema": "skilltape.dev/workflow/v1",
            "steps": steps,
        }))
        .expect("workflow JSON"),
    )
    .expect("workflow");
    fs::write(
        package_root.join("permissions.json"),
        serde_json::to_vec(&permission_document).expect("permissions JSON"),
    )
    .expect("permissions");
    let package = SkillPackage::load(&package_root).expect("loaded package");
    PackageFixture {
        _root: root,
        package,
    }
}

fn file_copy_package() -> PackageFixture {
    package(
        vec![json!({
            "action": "file",
            "id": "copy",
            "operation": "copy",
            "from": "inputs/source.txt",
            "to": "outputs/result.txt"
        })],
        permissions(&["inputs/**"], &["outputs/**"], &[]),
    )
}

fn limits() -> ResourceLimits {
    ResourceLimits {
        max_processes: 1,
        step_timeout: Duration::from_secs(2),
        max_output_bytes: 64,
    }
}

fn input_root(root: &Path) -> std::path::PathBuf {
    let input = root.join("input");
    fs::create_dir(&input).expect("input");
    fs::write(input.join("source.txt"), "payload").expect("source");
    input
}

async fn verify_fixture(
    fixture: PackageFixture,
    input_root: &Path,
    output_root: &Path,
    assertions: Vec<Assertion>,
) -> skilltape_verify::Receipt {
    verify_run(VerifyRequest {
        package: fixture.package,
        input_root: input_root.to_owned(),
        output_root: output_root.to_owned(),
        limits: limits(),
        assertions,
    })
    .await
    .expect("verification")
}

#[tokio::test]
async fn verifies_files_hashes_text_and_policy_decisions_without_raw_output() {
    let root = tempdir().expect("root");
    let input = input_root(root.path());
    let receipt = verify_fixture(
        file_copy_package(),
        &input,
        &root.path().join("output"),
        vec![
            Assertion::FileExists {
                path: "outputs/result.txt".into(),
            },
            Assertion::FileHash {
                path: "result.txt".into(),
                sha256: "239f59ed55e737c77147cf55ad0c1b030b6d7ee748a7426952f9b852d5a935e5".into(),
            },
            Assertion::FileTextContains {
                path: "result.txt".into(),
                text: "payload-secret-value".into(),
            },
        ],
    )
    .await;

    assert_eq!(receipt.status, ReceiptStatus::AssertionFailed);
    assert!(receipt
        .policy_decisions
        .iter()
        .all(|decision| decision.allowed));
    let document = serde_json::to_string(&receipt).expect("receipt JSON");
    assert!(!document.contains("payload-secret-value"));
    assert!(!document.contains("payload"));
    assert_eq!(receipt.steps.len(), 1);
    assert_eq!(receipt.steps[0].stdout_bytes, 0);
}

#[tokio::test]
async fn successful_receipt_is_deterministic_and_schema_validated() {
    let first_root = tempdir().expect("first root");
    let first_input = input_root(first_root.path());
    let first = verify_fixture(
        file_copy_package(),
        &first_input,
        &first_root.path().join("output"),
        vec![Assertion::FileTextContains {
            path: "result.txt".into(),
            text: "payload".into(),
        }],
    )
    .await;

    let second_root = tempdir().expect("second root");
    let second_input = input_root(second_root.path());
    let second = verify_fixture(
        file_copy_package(),
        &second_input,
        &second_root.path().join("output"),
        vec![Assertion::FileTextContains {
            path: "result.txt".into(),
            text: "payload".into(),
        }],
    )
    .await;

    assert_eq!(first.status, ReceiptStatus::Succeeded);
    assert_eq!(first, second);
    let document = serde_json::to_value(&first).expect("receipt value");
    let schema: Value = serde_json::from_str(include_str!("../../../schemas/receipt/v1.json"))
        .expect("receipt schema");
    let validator = jsonschema::validator_for(&schema).expect("validator");
    assert!(validator.iter_errors(&document).next().is_none());

    let mut invalid = document;
    invalid
        .as_object_mut()
        .expect("receipt object")
        .remove("skill_hash");
    assert!(validator.iter_errors(&invalid).next().is_some());
}

#[tokio::test]
async fn assertion_failures_and_missing_files_are_receipt_results() {
    let root = tempdir().expect("root");
    let input = input_root(root.path());
    let receipt = verify_fixture(
        file_copy_package(),
        &input,
        &root.path().join("output"),
        vec![
            Assertion::FileExists {
                path: "missing.txt".into(),
            },
            Assertion::FileHash {
                path: "result.txt".into(),
                sha256: "0000000000000000000000000000000000000000000000000000000000000000".into(),
            },
            Assertion::CommandExit {
                step_id: "not-present".into(),
                code: 0,
            },
        ],
    )
    .await;

    assert_eq!(receipt.status, ReceiptStatus::AssertionFailed);
    assert_eq!(receipt.assertions.len(), 3);
    assert!(receipt.assertions.iter().all(|assertion| !assertion.passed));
}

#[tokio::test]
async fn unsafe_assertion_paths_are_rejected_before_running() {
    let root = tempdir().expect("root");
    let input = input_root(root.path());
    let result = verify_run(VerifyRequest {
        package: file_copy_package().package,
        input_root: input,
        output_root: root.path().join("output"),
        limits: limits(),
        assertions: vec![Assertion::FileExists {
            path: "../outside".into(),
        }],
    })
    .await;

    assert!(matches!(result, Err(VerifyError::InvalidAssertion { .. })));
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn command_exit_assertion_is_recorded_for_a_real_run() {
    let root = tempdir().expect("root");
    let input = input_root(root.path());
    let fixture = package(
        vec![json!({
            "action": "exec",
            "id": "print",
            "program": "/usr/bin/printf",
            "args": ["hello"],
            "timeout_ms": 1000
        })],
        permissions(&[], &[], &["/usr/bin/printf"]),
    );
    let receipt = verify_fixture(
        fixture,
        &input,
        &root.path().join("output"),
        vec![Assertion::CommandExit {
            step_id: "print".into(),
            code: 0,
        }],
    )
    .await;
    assert_eq!(receipt.status, ReceiptStatus::Succeeded);
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn command_exit_mismatch_is_a_failed_assertion() {
    let root = tempdir().expect("root");
    let input = input_root(root.path());
    let fixture = package(
        vec![json!({
            "action": "exec",
            "id": "print",
            "program": "/usr/bin/printf",
            "args": ["hello"],
            "timeout_ms": 1000
        })],
        permissions(&[], &[], &["/usr/bin/printf"]),
    );
    let receipt = verify_fixture(
        fixture,
        &input,
        &root.path().join("output"),
        vec![Assertion::CommandExit {
            step_id: "print".into(),
            code: 1,
        }],
    )
    .await;
    assert_eq!(receipt.status, ReceiptStatus::AssertionFailed);
    assert!(!receipt.assertions[0].passed);
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn receipt_preserves_per_step_output_truncation() {
    let root = tempdir().expect("root");
    let input = input_root(root.path());
    let fixture = package(
        vec![json!({
            "action": "exec",
            "id": "print",
            "program": "/usr/bin/printf",
            "args": ["0123456789"],
            "timeout_ms": 1000
        })],
        permissions(&[], &[], &["/usr/bin/printf"]),
    );
    let receipt = verify_run(VerifyRequest {
        package: fixture.package,
        input_root: input,
        output_root: root.path().join("output"),
        limits: ResourceLimits {
            max_processes: 1,
            step_timeout: Duration::from_secs(2),
            max_output_bytes: 4,
        },
        assertions: vec![Assertion::CommandExit {
            step_id: "print".into(),
            code: 0,
        }],
    })
    .await
    .expect("verification");

    assert_eq!(receipt.status, ReceiptStatus::Succeeded);
    assert_eq!(receipt.steps[0].stdout_bytes, 4);
    assert!(receipt.steps[0].stdout_truncated);
    assert!(!receipt.steps[0].stderr_truncated);
}

#[cfg(unix)]
#[tokio::test]
async fn symlinked_output_assertion_is_rejected() {
    let root = tempdir().expect("root");
    let input = input_root(root.path());
    let output = root.path().join("output");
    let outside = root.path().join("outside");
    fs::create_dir(&output).expect("output");
    fs::create_dir(&outside).expect("outside");
    std::os::unix::fs::symlink(&outside, output.join("linked")).expect("symlink");

    let result = verify_run(VerifyRequest {
        package: package(Vec::new(), permissions(&[], &[], &[])).package,
        input_root: input,
        output_root: output,
        limits: limits(),
        assertions: vec![Assertion::FileExists {
            path: "linked/file.txt".into(),
        }],
    })
    .await;

    assert!(matches!(result, Err(VerifyError::AssertionInput { .. })));
}

#[cfg(unix)]
#[tokio::test]
async fn missing_output_root_through_symlink_is_rejected() {
    let root = tempdir().expect("root");
    let input = input_root(root.path());
    let outside = root.path().join("outside");
    let link = root.path().join("link");
    fs::create_dir(&outside).expect("outside");
    std::os::unix::fs::symlink(&outside, &link).expect("symlink");

    let result = verify_run(VerifyRequest {
        package: package(Vec::new(), permissions(&[], &[], &[])).package,
        input_root: input,
        output_root: link.join("missing-output"),
        limits: limits(),
        assertions: vec![Assertion::FileExists {
            path: "file.txt".into(),
        }],
    })
    .await;

    assert!(matches!(result, Err(VerifyError::AssertionInput { .. })));
}

#[cfg(unix)]
#[tokio::test]
async fn existing_output_root_through_symlink_is_rejected() {
    let root = tempdir().expect("root");
    let input = input_root(root.path());
    let outside = root.path().join("outside");
    let link = root.path().join("link");
    fs::create_dir(&outside).expect("outside");
    std::os::unix::fs::symlink(&outside, &link).expect("symlink");
    let output = link.join("existing-output");
    fs::create_dir(&output).expect("output");

    let result = verify_run(VerifyRequest {
        package: package(Vec::new(), permissions(&[], &[], &[])).package,
        input_root: input,
        output_root: output,
        limits: limits(),
        assertions: vec![Assertion::FileExists {
            path: "file.txt".into(),
        }],
    })
    .await;

    assert!(matches!(result, Err(VerifyError::AssertionInput { .. })));
}
