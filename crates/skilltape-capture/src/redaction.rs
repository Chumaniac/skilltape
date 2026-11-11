use std::cmp::Ordering;
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
    Regex::new(
        r"\b(?:(?:sk|pk|rk)[_-](?:live|test)[_-][A-Za-z0-9_-]{8,}|sk-proj-[A-Za-z0-9_-]{8,}|(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})\b",
    )
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

#[derive(Debug, Eq, PartialEq)]
struct Match {
    start: usize,
    end: usize,
    name: String,
}

#[derive(Debug, Eq, PartialEq)]
struct QueuedMatch {
    found: Match,
    source: usize,
}

impl Ord for QueuedMatch {
    fn cmp(&self, other: &Self) -> Ordering {
        self.found
            .start
            .cmp(&other.found.start)
            .then_with(|| other.found.end.cmp(&self.found.end))
            .then_with(|| self.found.name.cmp(&other.found.name))
            .then_with(|| self.source.cmp(&other.source))
    }
}

impl PartialOrd for QueuedMatch {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Removes known and configured secrets while incrementally bounding the
/// sanitized result without splitting a UTF-8 code point.
pub fn redact_text(input: &str, config: &RedactionConfig) -> RedactedText {
    let configured_names = configured_name_patterns(&config.secret_names);
    let mut match_sources: Vec<Box<dyn Iterator<Item = Match> + '_>> = Vec::new();
    match_sources.push(Box::new(NAMED_SECRET.captures_iter(input).filter_map(
        |captures| {
            let name = captures.name("name")?;
            let secret = captures.name("secret")?;
            Some(Match {
                start: secret.start(),
                end: secret.end(),
                name: normalize_name(name.as_str()),
            })
        },
    )));
    match_sources.push(Box::new(BEARER_TOKEN.captures_iter(input).filter_map(
        |captures| {
            let secret = captures.name("secret")?;
            Some(Match {
                start: secret.start(),
                end: secret.end(),
                name: "bearer_token".to_owned(),
            })
        },
    )));
    match_sources.push(Box::new(STANDALONE_API_KEY.find_iter(input).map(|found| {
        Match {
            start: found.start(),
            end: found.end(),
            name: "api_key".to_owned(),
        }
    })));
    for (name, pattern) in &configured_names {
        match_sources.push(Box::new(pattern.captures_iter(input).filter_map(
            move |captures| {
                let secret = captures.name("secret")?;
                Some(Match {
                    start: secret.start(),
                    end: secret.end(),
                    name: name.clone(),
                })
            },
        )));
    }
    for (index, pattern) in config.patterns.iter().enumerate() {
        let name = format!("configured_pattern_{index}");
        match_sources.push(Box::new(pattern.find_iter(input).map(move |found| Match {
            start: found.start(),
            end: found.end(),
            name: name.clone(),
        })));
    }

    let mut pending = BTreeSet::new();
    for (source, matches) in match_sources.iter_mut().enumerate() {
        if let Some(found) = matches.next() {
            pending.insert(QueuedMatch { found, source });
        }
    }

    let mut sanitized = String::with_capacity(input.len().min(config.max_output_bytes));
    let mut redactions = Vec::new();
    let mut cursor = 0;
    let mut truncated = false;
    while let Some(QueuedMatch { found, source }) = pending.pop_first() {
        if let Some(next) = match_sources[source].next() {
            pending.insert(QueuedMatch {
                found: next,
                source,
            });
        }
        if found.start < cursor || found.start == found.end {
            continue;
        }

        if !append_bounded(
            &mut sanitized,
            &input[cursor..found.start],
            config.max_output_bytes,
        ) {
            truncated = true;
            break;
        }
        if sanitized.len() == config.max_output_bytes {
            truncated = true;
            break;
        }

        let secret = &input[found.start..found.end];
        let metadata = RedactionMetadata {
            name: found.name,
            original_bytes: secret.len(),
            sha256: sha256_hex(secret.as_bytes()),
        };
        let replacement_complete =
            append_redaction_marker(&mut sanitized, &metadata, config.max_output_bytes);
        redactions.push(metadata);
        cursor = found.end;
        if !replacement_complete {
            truncated = true;
            break;
        }
    }

    if !truncated && !append_bounded(&mut sanitized, &input[cursor..], config.max_output_bytes) {
        truncated = true;
    }

    RedactedText {
        text: sanitized,
        redactions,
        original_bytes: input.len(),
        truncated,
    }
}

fn configured_name_patterns(names: &BTreeSet<String>) -> Vec<(String, Regex)> {
    names
        .iter()
        .filter(|name| !name.is_empty())
        .filter_map(|name| {
            let expression = format!(
                r#"(?i)\b{}\b\s*(?:=|:)\s*["']?(?P<secret>[^\s"'&;,}}\]]+)"#,
                regex::escape(name)
            );
            Regex::new(&expression)
                .ok()
                .map(|pattern| (normalize_name(name), pattern))
        })
        .collect()
}

fn normalize_name(name: &str) -> String {
    name.to_lowercase().replace('-', "_")
}

fn append_bounded(output: &mut String, value: &str, max_bytes: usize) -> bool {
    let remaining = max_bytes.saturating_sub(output.len());
    if value.len() <= remaining {
        output.push_str(value);
        return true;
    }

    let mut boundary = remaining;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    output.push_str(&value[..boundary]);
    false
}

fn append_redaction_marker(
    output: &mut String,
    metadata: &RedactionMetadata,
    max_bytes: usize,
) -> bool {
    let original_bytes = metadata.original_bytes.to_string();
    for part in [
        "[REDACTED name=",
        metadata.name.as_str(),
        " bytes=",
        original_bytes.as_str(),
        " sha256=",
        metadata.sha256.as_str(),
        "]",
    ] {
        if !append_bounded(output, part, max_bytes) {
            return false;
        }
    }
    true
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
