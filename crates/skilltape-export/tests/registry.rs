use skilltape_export::{exporter_for, supported_targets, RegistryError};

#[test]
fn registry_exposes_the_supported_targets_in_stable_order() {
    assert_eq!(
        supported_targets(),
        &["generic", "generic-agent-skill", "claude-code", "codex", "cursor"]
    );
}

#[test]
fn registry_returns_exporters_by_id() {
    assert_eq!(
        exporter_for("generic-agent-skill")
            .expect("generic exporter")
            .target_id(),
        "generic-agent-skill"
    );
    assert_eq!(
        exporter_for("generic").expect("generic alias").target_id(),
        "generic-agent-skill"
    );
    assert_eq!(
        exporter_for("claude-code")
            .expect("Claude exporter")
            .target_id(),
        "claude-code"
    );
    assert_eq!(
        exporter_for("codex").expect("codex exporter").target_id(),
        "codex"
    );
    assert_eq!(
        exporter_for("cursor").expect("cursor exporter").target_id(),
        "cursor"
    );
}

#[test]
fn registry_rejects_unknown_target_without_a_fallback() {
    let error = match exporter_for("future-target") {
        Ok(_) => panic!("unknown target must fail"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        RegistryError::UnknownTarget {
            target: "future-target".to_owned(),
            supported: "generic, generic-agent-skill, claude-code, codex, cursor".to_owned(),
        }
    );
}
