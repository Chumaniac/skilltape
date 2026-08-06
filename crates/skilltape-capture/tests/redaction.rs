use std::collections::BTreeSet;

use regex::Regex;
use skilltape_capture::{redact_text, snapshot_environment, RedactionConfig};

const API_KEY: &str = "sk-live-raw-fixture-123456";
const BEARER_TOKEN: &str = "eyJhbGciOiJIUzI1NiJ9.raw.signature";
const PASSWORD: &str = "correct-horse-battery-staple";
const CUSTOM_SECRET: &str = "custom-raw-secret-42";
const PATTERN_SECRET: &str = "vault://production/payment-token";

fn config() -> RedactionConfig {
    RedactionConfig {
        secret_names: BTreeSet::from(["private_note".to_owned()]),
        patterns: vec![Regex::new(r"vault://[^\s]+").expect("valid fixture regex")],
        max_output_bytes: 16 * 1024,
    }
}

#[test]
fn redacts_built_in_and_configured_secret_forms_without_retaining_plaintext() {
    let input = format!(
        "api_key={API_KEY}\nAuthorization: Bearer {BEARER_TOKEN}\npassword={PASSWORD}\nprivate_note={CUSTOM_SECRET}\nreference={PATTERN_SECRET}"
    );

    let redacted = redact_text(&input, &config());
    let observable = format!("{}\n{redacted:?}", redacted.text);

    for raw_secret in [
        API_KEY,
        BEARER_TOKEN,
        PASSWORD,
        CUSTOM_SECRET,
        PATTERN_SECRET,
    ] {
        assert!(!observable.contains(raw_secret), "raw secret leaked");
    }
    for name in [
        "api_key",
        "bearer_token",
        "password",
        "private_note",
        "configured_pattern_0",
    ] {
        assert!(observable.contains(name), "missing redaction name {name}");
    }
    assert_eq!(redacted.redactions.len(), 5);
    assert!(redacted
        .redactions
        .iter()
        .all(|item| item.sha256.len() == 64 && item.original_bytes > 0));
}

#[test]
fn truncates_redacted_output_at_a_unicode_boundary() {
    let config = RedactionConfig {
        max_output_bytes: 7,
        ..RedactionConfig::default()
    };

    let redacted = redact_text("ééééSECRET", &config);

    assert_eq!(redacted.text, "ééé");
    assert_eq!(redacted.text.len(), 6);
    assert!(redacted.truncated);
    assert_eq!(redacted.original_bytes, 14);
}

#[test]
fn environment_snapshot_is_empty_by_default_and_metadata_only_when_allowlisted() {
    const ALLOWED_NAME: &str = "SKILLTAPE_CAPTURE_ALLOWED_FIXTURE";
    const DISALLOWED_NAME: &str = "SKILLTAPE_CAPTURE_DISALLOWED_FIXTURE";
    const ALLOWED_VALUE: &str = "allowed-but-still-sensitive";
    const DISALLOWED_VALUE: &str = "must-never-be-read";

    std::env::set_var(ALLOWED_NAME, ALLOWED_VALUE);
    std::env::set_var(DISALLOWED_NAME, DISALLOWED_VALUE);

    let empty = snapshot_environment(&[]);
    let snapshot = snapshot_environment(&[ALLOWED_NAME.to_owned()]);
    let observable = format!("{snapshot:?}");

    std::env::remove_var(ALLOWED_NAME);
    std::env::remove_var(DISALLOWED_NAME);

    assert!(empty.variables.is_empty());
    assert_eq!(snapshot.variables.len(), 1);
    let metadata = snapshot
        .variables
        .get(ALLOWED_NAME)
        .expect("allowlisted variable is recorded");
    assert_eq!(metadata.name, ALLOWED_NAME);
    assert_eq!(metadata.original_bytes, ALLOWED_VALUE.len());
    assert_eq!(metadata.sha256.len(), 64);
    assert!(!observable.contains(ALLOWED_VALUE));
    assert!(!observable.contains(DISALLOWED_NAME));
    assert!(!observable.contains(DISALLOWED_VALUE));
}
