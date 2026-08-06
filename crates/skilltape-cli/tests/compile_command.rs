use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use serde_json::{json, Value};
use skilltape_core::SkillPackage;
use skilltape_tape::{
    EventSource, RedactionState, TapeEvent, TapeEventKind, TapeManifest, TapeStore, TAPE_SCHEMA_V1,
};
use tempfile::TempDir;

fn create_tape(root: &Path) {
    let store = TapeStore::create(
        root,
        TapeManifest {
            schema: TAPE_SCHEMA_V1.to_owned(),
            id: "compile-test".to_owned(),
            started_at_ms: 1,
            finished_at_ms: None,
            platform: "test".to_owned(),
            workspace_root: "workspace".to_owned(),
            event_count: 0,
        },
    )
    .expect("create tape");

    store
        .append(&TapeEvent {
            sequence: 0,
            occurred_at_ms: 2,
            kind: TapeEventKind::FilesystemChanged,
            source: EventSource::Filesystem,
            payload: json!({
                "kind": "created",
                "path": "src/main.rs",
                "content_hash": "hash-main",
                "size": 12
            }),
            redaction: RedactionState::Redacted,
        })
        .expect("append tape event");
    store.finish(3).expect("finish tape");
}

fn compile(tape: &Path, output: &Path, extra_args: &[&str]) -> assert_cmd::assert::Assert {
    let mut command = Command::cargo_bin("skilltape").expect("binary");
    command
        .args(["compile"])
        .arg(tape)
        .args(["--output"])
        .arg(output)
        .args(extra_args);
    command.assert()
}

fn package_files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut entries = fs::read_dir(current)
            .expect("read package directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read package entries");
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                collect(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root)
                        .expect("package-relative path")
                        .to_owned(),
                    fs::read(path).expect("read package file"),
                );
            }
        }
    }

    let mut files = BTreeMap::new();
    collect(root, root, &mut files);
    files
}

#[test]
fn compile_materializes_a_lintable_package_with_provenance() {
    let temp = TempDir::new().expect("temp directory");
    let tape = temp.path().join("tape");
    let output = temp.path().join("compiled-skill");
    create_tape(&tape);

    compile(&tape, &output, &[])
        .success()
        .stderr(predicates::str::is_empty());

    let package = SkillPackage::load(&output).expect("compiled package should load");
    let report = package.lint(false);
    assert!(report.errors.is_empty(), "lint errors: {:?}", report.errors);
    assert!(
        report.warnings.is_empty(),
        "lint warnings: {:?}",
        report.warnings
    );
    assert!(output.join("SKILL.md").is_file());
    assert!(output.join("workflow.yaml").is_file());
    assert!(output.join("permissions.json").is_file());
    assert!(output.join("fixtures/changes/0001.json").is_file());

    let provenance: Value =
        serde_json::from_slice(&fs::read(output.join("compile.json")).expect("compile JSON"))
            .expect("valid provenance JSON");
    assert_eq!(provenance["schema"], "skilltape.dev/compile/v1");
    assert_eq!(provenance["target_identity"], "generic-agent-skill@0.1.0");
    assert_eq!(provenance["steps"][0]["event_sequences"], json!([0]));
}

#[test]
fn compile_repeating_the_same_tape_is_byte_deterministic() {
    let temp = TempDir::new().expect("temp directory");
    let tape = temp.path().join("tape");
    let first = temp.path().join("first/compiled-skill");
    let second = temp.path().join("second/compiled-skill");
    create_tape(&tape);

    compile(&tape, &first, &[]).success();
    compile(&tape, &second, &[]).success();

    assert_eq!(package_files(&first), package_files(&second));
}

#[test]
fn compile_rejects_existing_output_without_changing_it() {
    let temp = TempDir::new().expect("temp directory");
    let tape = temp.path().join("tape");
    let output = temp.path().join("existing");
    create_tape(&tape);
    fs::create_dir_all(&output).expect("existing output");
    fs::write(output.join("keep.txt"), b"keep this exact file").expect("marker");

    compile(&tape, &output, &[])
        .code(2)
        .stderr(predicates::str::contains("already exists"));

    assert_eq!(
        fs::read(output.join("keep.txt")).expect("marker"),
        b"keep this exact file"
    );
    assert!(!output.join("SKILL.md").exists());
}

#[cfg(unix)]
#[test]
fn compile_rejects_existing_symlink_output_without_following_it() {
    let temp = TempDir::new().expect("temp directory");
    let tape = temp.path().join("tape");
    let outside = temp.path().join("outside");
    let output = temp.path().join("linked-output");
    create_tape(&tape);
    fs::create_dir_all(&outside).expect("outside");
    fs::write(outside.join("keep.txt"), b"outside marker").expect("outside marker");
    std::os::unix::fs::symlink(&outside, &output).expect("output symlink");

    compile(&tape, &output, &[])
        .code(2)
        .stderr(predicates::str::contains("already exists"));

    assert_eq!(
        fs::read(outside.join("keep.txt")).expect("outside marker"),
        b"outside marker"
    );
    assert!(output
        .symlink_metadata()
        .expect("symlink metadata")
        .file_type()
        .is_symlink());
}

#[test]
fn compile_reports_invalid_tape_as_input_failure_without_output() {
    let temp = TempDir::new().expect("temp directory");
    let tape = temp.path().join("missing-tape");
    let output = temp.path().join("compiled-skill");

    compile(&tape, &output, &[])
        .code(2)
        .stderr(predicates::str::contains("tape"));

    assert!(!output.exists());
}

#[test]
fn compile_provider_request_is_explicitly_offline_and_does_not_publish() {
    let temp = TempDir::new().expect("temp directory");
    let tape = temp.path().join("tape");
    let output = temp.path().join("compiled-skill");
    create_tape(&tape);

    compile(&tape, &output, &["--provider", "offline"])
        .code(3)
        .stderr(predicates::str::contains("provider").and(predicates::str::contains("offline")));

    assert!(!output.exists());
}

#[test]
fn compile_accept_proposal_without_provider_is_a_policy_failure() {
    let temp = TempDir::new().expect("temp directory");
    let tape = temp.path().join("tape");
    let output = temp.path().join("compiled-skill");
    create_tape(&tape);

    compile(&tape, &output, &["--accept-proposal"])
        .code(3)
        .stderr(predicates::str::contains("provider"));

    assert!(!output.exists());
}
