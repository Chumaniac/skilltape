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
struct ManifestFields { schema: String, id: String, started_at_ms: u64, finished_at_ms: Option<u64>, platform: String, workspace_root: String, event_count: u64 }

impl<'de> Deserialize<'de> for TapeManifest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let fields = ManifestFields::deserialize(deserializer)?;
        if fields.schema != TAPE_SCHEMA_V1 { return Err(serde::de::Error::custom("invalid tape schema")); }
        if fields.id.is_empty() { return Err(serde::de::Error::custom("manifest id must not be empty")); }
        if fields.workspace_root.starts_with('/') { return Err(serde::de::Error::custom("workspace_root must be relative")); }
        Ok(Self { schema: fields.schema, id: fields.id, started_at_ms: fields.started_at_ms, finished_at_ms: fields.finished_at_ms, platform: fields.platform, workspace_root: fields.workspace_root, event_count: fields.event_count })
    }
}
