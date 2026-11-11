use serde::{Deserialize, Serialize};

use crate::TAPE_SCHEMA_V1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TapeManifest {
    pub schema: String,
    pub id: String,
    pub started_at_ms: u64,
    pub finished_at_ms: Option<u64>,
    pub platform: String,
    pub workspace_root: String,
    pub event_count: u64,
}

#[derive(Deserialize)]
struct ManifestFields {
    schema: String,
    id: String,
    started_at_ms: u64,
    finished_at_ms: Option<u64>,
    platform: String,
    workspace_root: String,
    event_count: u64,
}

fn is_workspace_relative(root: &str) -> bool {
    if root.starts_with('/') || root.starts_with('\\') {
        return false;
    }

    let bytes = root.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
    {
        return false;
    }

    !root.split(['/', '\\']).any(|segment| segment == "..")
}

impl<'de> Deserialize<'de> for TapeManifest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let fields = ManifestFields::deserialize(deserializer)?;
        if fields.schema != TAPE_SCHEMA_V1 {
            return Err(serde::de::Error::custom("invalid tape schema"));
        }
        if fields.id.is_empty() {
            return Err(serde::de::Error::custom("manifest id must not be empty"));
        }
        if !is_workspace_relative(&fields.workspace_root) {
            return Err(serde::de::Error::custom("workspace_root must be relative"));
        }
        Ok(Self {
            schema: fields.schema,
            id: fields.id,
            started_at_ms: fields.started_at_ms,
            finished_at_ms: fields.finished_at_ms,
            platform: fields.platform,
            workspace_root: fields.workspace_root,
            event_count: fields.event_count,
        })
    }
}
