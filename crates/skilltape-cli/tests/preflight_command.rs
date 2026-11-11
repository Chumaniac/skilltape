use std::fs;
use std::path::Path;

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::{json, Value};
use skilltape_core::create_skill_template;
use tempfile::tempdir;

#[test]
fn preflight_is_read_only_and_does_not_create_a_receipt_or_execute_arguments() {
    let temp = tempdir().unwrap();
    let skill = temp.path().join("skill");
    let marker = temp.path().join("must-not-be-created");
    create_skill_template(&skill, "preflight-test").unwrap();
    fs::write(skill.join("workflow.yaml"), serde_json::to_vec(&json!({"schema":"skilltape.dev/workflow/v1","steps":[{"id":"probe","action":"exec","program":"sh","timeout_ms":1000,"args":["-c",format!("touch {}",marker.display())]}]})).unwrap()).unwrap();
    let before = fs::read(skill.join("workflow.yaml")).unwrap();
    let output = cargo_bin_cmd!("skilltape")
        .args(["preflight", skill.to_str().unwrap(), "--json"])
        .assert()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let plan: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(plan["execution"], "not-run");
    assert_eq!(plan["allowed"], false);
    assert!(!marker.exists());
    assert!(!String::from_utf8(output).unwrap().contains("touch"));
    assert_eq!(fs::read(skill.join("workflow.yaml")).unwrap(), before);
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    assert!(!skill.join(".skilltape").exists());
}

#[test]
fn preflight_has_distinct_allowed_and_invalid_exit_codes() {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/minimal-skill");
    let output = cargo_bin_cmd!("skilltape")
        .arg("preflight")
        .arg(example)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plan: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(plan["allowed"], true);
    cargo_bin_cmd!("skilltape")
        .args(["preflight", "missing-preflight-fixture", "--json"])
        .assert()
        .code(2)
        .stdout("");
}

#[test]
fn human_preflight_escapes_untrusted_terminal_control_labels() {
    for label in ["bad\u{1b}[2J\nFORGED", "bad\u{202e}FORGED"] {
        let temp = tempdir().unwrap();
        let skill = temp.path().join("skill");
        create_skill_template(&skill, "control-probe").unwrap();
        fs::write(skill.join("workflow.yaml"), serde_json::to_vec(&json!({"schema":"skilltape.dev/workflow/v1","steps":[{"action":"exec","id":label,"program":"echo","args":["synthetic"],"timeout_ms":1000}]})).unwrap()).unwrap();
        let output = cargo_bin_cmd!("skilltape")
            .arg("preflight")
            .arg(&skill)
            .assert()
            .code(3)
            .get_output()
            .stdout
            .clone();
        let text = String::from_utf8(output).unwrap();
        assert!(!text.contains('\u{1b}'));
        assert!(!text.contains('\u{202e}'));
        assert!(!text.contains("\nFORGED"));
        assert!(text.contains("FORGED"));
    }
}
