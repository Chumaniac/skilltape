use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;
use sha2::{Digest, Sha256};

const DEFAULT_MAX_OUTPUT_BYTES: usize = 64 * 1024;

static NAMED_SECRET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?ix)
        \b(?P<name>api[_-]?key|access[_-]?token|auth[_-]?token|password|passwd|pwd|secret)
        \b\s*(?:=|:)\s*["']?(?P<secret>[^\s"'&;,}\]]+)
        "#,
    )
    .expect("built-in named-secret regex is valid")
});

static BEARER_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bBearer\s+(?P<secret>[A-Za-z0-9._~+/=-]+)")
        .expect("built-in bearer regex is valid")
});

static STANDALONE_API_KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:sk|pk|rk)[_-](?:live|test)[_-][A-Za-z0-9_-]{8,}\b")
        .expect("built-in API-key regex is valid")
});

/// Redaction rules applied before captured text is persisted.
///
/// `secret_names` are matched as case-insensitive assignment or parameter
/// names. Each configured pattern treats its complete match as secret text.
pub struct RedactionConfig {
    pub secret_names: BTreeSet<String>,
    pub patterns: Vec<Regex>,
    pub max_output_bytes: usize,
}

impl Default for RedactionConfig {
    fn default() -> Self {
        Self {
            secret_names: BTreeSet::new(),
            patterns: Vec::new(),
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        }
    }
}

/// Metadata for one removed secret. Secret plaintext is never retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedactionMetadata {
    pub name: String,
    pub original_bytes: usize,
    pub sha256: String,
}

/// Sanitized, byte-bounded capture text and its non-secret metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedactedText {
    pub text: String,
    pub redactions: Vec<RedactionMetadata>,
    pub original_bytes: usize,
    pub truncated: bool,
}

#[derive(Debug)]
struct Match {
    start: usize,
    end: usize,
    name: String,
}

/// Removes known and configured secrets, then truncates the sanitized result
/// without splitting a UTF-8 code point.
pub fn redact_text(input: &str, config: &RedactionConfig) -> RedactedText {
    let mut matches = built_in_matches(input);
    matches.extend(configured_name_matches(input, &config.secret_names));
    for (index, pattern) in config.patterns.iter().enumerate() {
        matches.extend(pattern.find_iter(input).map(|found| Match {
            start: found.start(),
            end: found.end(),
            name: format!("configured_pattern_{index}"),
        }));
    }

    matches.sort_by_key(|found| (found.start, Reverse(found.end), found.name.clone()));

    let mut sanitized = String::with_capacity(input.len().min(config.max_output_bytes));
    let mut redactions = Vec::new();
    let mut cursor = 0;
    for found in matches {
        if found.start < cursor || found.start == found.end {
            continue;
        }

        sanitized.push_str(&input[cursor..found.start]);
        let secret = &input[found.start..found.end];
        let metadata = RedactionMetadata {
            name: found.name,
            original_bytes: secret.len(),
            sha256: sha256_hex(secret.as_bytes()),
        };
        sanitized.push_str(&format!(
            "[REDACTED name={} bytes={} sha256={}]",
            metadata.name, metadata.original_bytes, metadata.sha256
        ));
        redactions.push(metadata);
        cursor = found.end;
    }
    sanitized.push_str(&input[cursor..]);

    let truncated = sanitized.len() > config.max_output_bytes;
    if truncated {
        truncate_utf8(&mut sanitized, config.max_output_bytes);
    }

    RedactedText {
        text: sanitized,
        redactions,
        original_bytes: input.len(),
        truncated,
    }
}

fn built_in_matches(input: &str) -> Vec<Match> {
    let mut matches = Vec::new();
    matches.extend(NAMED_SECRET.captures_iter(input).filter_map(|captures| {
        let name = captures.name("name")?;
        let secret = captures.name("secret")?;
        Some(Match {
            start: secret.start(),
            end: secret.end(),
            name: normalize_name(name.as_str()),
        })
    }));
    matches.extend(BEARER_TOKEN.captures_iter(input).filter_map(|captures| {
        let secret = captures.name("secret")?;
        Some(Match {
            start: secret.start(),
            end: secret.end(),
            name: "bearer_token".to_owned(),
        })
    }));
    matches.extend(STANDALONE_API_KEY.find_iter(input).map(|found| Match {
        start: found.start(),
        end: found.end(),
        name: "api_key".to_owned(),
    }));
    matches
}

fn configured_name_matches(input: &str, names: &BTreeSet<String>) -> Vec<Match> {
    let mut matches = Vec::new();
    for name in names {
        if name.is_empty() {
            continue;
        }
        let expression = format!(
            r#"(?i)\b{}\b\s*(?:=|:)\s*["']?(?P<secret>[^\s"'&;,}}\]]+)"#,
            regex::escape(name)
        );
        let Ok(pattern) = Regex::new(&expression) else {
            continue;
        };
        matches.extend(pattern.captures_iter(input).filter_map(|captures| {
            let secret = captures.name("secret")?;
            Some(Match {
                start: secret.start(),
                end: secret.end(),
                name: normalize_name(name),
            })
        }));
    }
    matches
}

fn normalize_name(name: &str) -> String {
    name.to_lowercase().replace('-', "_")
}

fn truncate_utf8(value: &mut String, max_bytes: usize) {
    let mut boundary = max_bytes.min(value.len());
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
}

pub(crate) fn sha256_hex(value: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(value);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}
