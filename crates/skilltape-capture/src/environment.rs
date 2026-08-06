use std::collections::BTreeMap;

use crate::redaction::sha256_hex;

/// Non-secret metadata for one explicitly allowlisted environment variable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentVariable {
    pub name: String,
    pub original_bytes: usize,
    pub sha256: String,
}

/// Deterministically ordered metadata for explicitly allowlisted variables.
///
/// Values are hashed and discarded immediately; this type cannot expose their
/// plaintext. Missing and non-Unicode variables are omitted.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EnvironmentSnapshot {
    pub variables: BTreeMap<String, EnvironmentVariable>,
}

/// Reads only names in `allowlist` and returns metadata without retaining any
/// environment value plaintext.
pub fn snapshot_environment(allowlist: &[String]) -> EnvironmentSnapshot {
    let variables = allowlist
        .iter()
        .filter_map(|name| {
            let value = std::env::var(name).ok()?;
            let metadata = EnvironmentVariable {
                name: name.clone(),
                original_bytes: value.len(),
                sha256: sha256_hex(value.as_bytes()),
            };
            Some((name.clone(), metadata))
        })
        .collect();

    EnvironmentSnapshot { variables }
}
