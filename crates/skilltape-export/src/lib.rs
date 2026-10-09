//! Deterministic generic SkillPackage export.

use std::io;
use std::path::PathBuf;

use serde::Serialize;
use skilltape_core::LoadedSkillPackage;
use thiserror::Error;

mod generic;
mod plugin;
mod receipt;
mod registry;
mod snapshot;
pub use receipt::export_with_receipt;

pub use claude_code::ClaudeCodeExporter;
pub use codex::CodexExporter;
pub use cursor::CursorExporter;
pub use generic::GenericExporter;
pub use plugin::{
    run_plugin, validate_plugin_manifest, validate_request, ExportRequest, PluginError,
    PluginExportManifest, PluginRun, EXPORT_MANIFEST_SCHEMA_V1, EXPORT_REQUEST_SCHEMA_V1,
    PLUGIN_INVALID_INPUT_EXIT_CODE, PLUGIN_POLICY_FAILURE_EXIT_CODE,
};
pub use registry::{exporter_for, supported_targets, RegistryError};

mod claude_code;
mod codex;
mod cursor;

/// Exporter boundary shared by generic and platform-specific targets.
pub trait Exporter {
    fn target_id(&self) -> &'static str;
    fn export(
        &self,
        package: &LoadedSkillPackage,
        output: &std::path::Path,
    ) -> Result<ExportManifest, ExportError>;
}

/// Deterministic description of one published export.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExportManifest {
    pub target: String,
    pub files: Vec<String>,
    pub package_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<ReceiptReference>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReceiptReference {
    pub run_id: String,
    pub skill_hash: String,
    pub receipt_sha256: String,
    pub provenance: &'static str,
}

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("export exceeds the bounded entry, depth or byte capacity")]
    Capacity,
    #[error("export source changed during the selected snapshot")]
    ChangedSource,
    #[error("Receipt must be bounded, unambiguous and internally consistent successful metadata")]
    InvalidReceipt,
    #[error("Receipt identity does not match the actual exported package bytes")]
    ReceiptMismatch,
    #[error("export package lint failed with {errors} error(s)")]
    Lint { errors: usize },
    #[error("export output already exists: {path}")]
    OutputExists { path: PathBuf },
    #[error("export output path is unsafe: {path}")]
    UnsafeOutput { path: PathBuf },
    #[error("export target is not declared by the package: {target}")]
    TargetNotDeclared { target: String },
    #[error("package name is unsafe for the target layout: {name}")]
    InvalidTargetName { name: String },
    #[error("export source path is unsafe: {path}")]
    UnsafeSource { path: String },
    #[error("export source file is missing: {path}")]
    MissingSource { path: String },
    #[error("export source contains a symlink: {path}")]
    SymlinkSource { path: PathBuf },
    #[error("export source is not a regular file: {path}")]
    UnsupportedSource { path: PathBuf },
    #[error("export I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}
