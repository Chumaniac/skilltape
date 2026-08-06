use thiserror::Error;

use crate::{ClaudeCodeExporter, Exporter, GenericExporter};

const SUPPORTED_TARGETS: [&str; 3] = ["generic", "generic-agent-skill", "claude-code"];

#[derive(Debug, Error, Eq, PartialEq)]
pub enum RegistryError {
    #[error("unknown export target `{target}`; supported targets: {supported}")]
    UnknownTarget { target: String, supported: String },
}

pub fn supported_targets() -> &'static [&'static str] {
    &SUPPORTED_TARGETS
}

pub fn exporter_for(target: &str) -> Result<Box<dyn Exporter>, RegistryError> {
    match target {
        "generic" | "generic-agent-skill" => Ok(Box::new(GenericExporter)),
        "claude-code" => Ok(Box::new(ClaudeCodeExporter)),
        _ => Err(RegistryError::UnknownTarget {
            target: target.to_owned(),
            supported: SUPPORTED_TARGETS.join(", "),
        }),
    }
}
