use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use skilltape_core::SkillPackage;
use tempfile::NamedTempFile;
use thiserror::Error;

pub const EXPORT_REQUEST_SCHEMA_V1: &str = "skilltape.dev/export-request/v1";
pub const EXPORT_MANIFEST_SCHEMA_V1: &str = "skilltape.dev/export-manifest/v1";
pub const PLUGIN_INVALID_INPUT_EXIT_CODE: i32 = 2;
pub const PLUGIN_POLICY_FAILURE_EXIT_CODE: i32 = 3;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExportRequest {
    pub schema: String,
    pub target: String,
    pub input_root: String,
    pub output_root: String,
    pub package_hash: String,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
}

impl ExportRequest {
    pub fn new(
        target: impl Into<String>,
        input_root: impl Into<String>,
        output_root: impl Into<String>,
        package_hash: impl Into<String>,
        required_capabilities: Vec<String>,
    ) -> Self {
        Self {
            schema: EXPORT_REQUEST_SCHEMA_V1.to_owned(),
            target: target.into(),
            input_root: input_root.into(),
            output_root: output_root.into(),
            package_hash: package_hash.into(),
            required_capabilities,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PluginExportManifest {
    pub schema: String,
    pub target: String,
    pub package_path: String,
    pub files: Vec<String>,
    pub package_hash: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug)]
pub struct PluginRun {
    pub manifest: PluginExportManifest,
    pub diagnostics: String,
}

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("plugin request schema is unsupported: {schema}")]
    UnsupportedRequestSchema { schema: String },
    #[error("plugin request is invalid: {reason}")]
    InvalidRequest { reason: String },
    #[error("plugin manifest schema is unsupported: {schema}")]
    UnsupportedManifestSchema { schema: String },
    #[error("plugin manifest target does not match the request")]
    TargetMismatch,
    #[error("plugin manifest package hash does not match the request")]
    PackageHashMismatch,
    #[error("plugin manifest is missing capability: {capability}")]
    MissingCapability { capability: String },
    #[error("plugin manifest path is unsafe: {path}")]
    UnsafeManifestPath { path: String },
    #[error("plugin manifest path is missing: {path}")]
    MissingManifestPath { path: String },
    #[error("plugin manifest path is not a regular file: {path}")]
    UnsupportedManifestPath { path: String },
    #[error("plugin manifest path is a symlink: {path}")]
    SymlinkManifestPath { path: String },
    #[error("plugin manifest contains a duplicate file: {path}")]
    DuplicateManifestPath { path: String },
    #[error("plugin output root is unsafe: {path}")]
    UnsafeOutputRoot { path: PathBuf },
    #[error("plugin output package failed to load")]
    PackageLoad,
    #[error("plugin output package lint failed with {errors} error(s)")]
    LintFailed { errors: usize },
    #[error("plugin request file could not be written: {0}")]
    RequestIo(#[source] io::Error),
    #[error("plugin process could not be started: {0}")]
    Spawn(#[source] io::Error),
    #[error("plugin returned invalid input (exit code 2)")]
    PluginInvalidInput,
    #[error("plugin returned a policy/export failure (exit code 3)")]
    PluginPolicyFailure,
    #[error("plugin process crashed with exit code {code:?}")]
    PluginCrashed { code: Option<i32> },
    #[error("plugin stdout is not a valid ExportManifest: {0}")]
    InvalidManifest(#[from] serde_json::Error),
}

pub fn validate_request(request: &ExportRequest) -> Result<(), PluginError> {
    if request.schema != EXPORT_REQUEST_SCHEMA_V1 {
        return Err(PluginError::UnsupportedRequestSchema {
            schema: request.schema.clone(),
        });
    }
    if request.target.is_empty() || !is_safe_target(&request.target) {
        return Err(PluginError::InvalidRequest {
            reason: "target must be a non-empty safe identifier".to_owned(),
        });
    }
    if request.input_root.is_empty()
        || request.output_root.is_empty()
        || !is_clean_absolute_path(Path::new(&request.input_root))
        || !is_clean_absolute_path(Path::new(&request.output_root))
    {
        return Err(PluginError::InvalidRequest {
            reason: "input_root and output_root must be absolute paths".to_owned(),
        });
    }
    if !is_sha256(&request.package_hash) {
        return Err(PluginError::InvalidRequest {
            reason: "package_hash must be a lowercase or uppercase SHA-256 digest".to_owned(),
        });
    }
    validate_capabilities(&request.required_capabilities).map_err(|capability| {
        PluginError::InvalidRequest {
            reason: format!("invalid capability `{capability}`"),
        }
    })
}

pub fn validate_plugin_manifest(
    request: &ExportRequest,
    manifest: &PluginExportManifest,
    output_root: &Path,
) -> Result<(), PluginError> {
    validate_request(request)?;
    if manifest.schema != EXPORT_MANIFEST_SCHEMA_V1 {
        return Err(PluginError::UnsupportedManifestSchema {
            schema: manifest.schema.clone(),
        });
    }
    if manifest.target != request.target {
        return Err(PluginError::TargetMismatch);
    }
    if manifest.package_hash != request.package_hash {
        return Err(PluginError::PackageHashMismatch);
    }
    for capability in &request.required_capabilities {
        if !manifest
            .capabilities
            .iter()
            .any(|value| value == capability)
        {
            return Err(PluginError::MissingCapability {
                capability: capability.clone(),
            });
        }
    }
    validate_capabilities(&manifest.capabilities).map_err(|capability| {
        PluginError::InvalidRequest {
            reason: format!("invalid capability `{capability}`"),
        }
    })?;

    let output_root = canonical_output_root(output_root)?;
    let package_root = resolve_relative_directory(&output_root, &manifest.package_path)?;
    let mut seen = BTreeSet::new();
    for relative in &manifest.files {
        if !seen.insert(relative) {
            return Err(PluginError::DuplicateManifestPath {
                path: relative.clone(),
            });
        }
        let path = resolve_relative_file(&output_root, relative)?;
        if !path.starts_with(&output_root) || !path.starts_with(&package_root) {
            return Err(PluginError::UnsafeManifestPath {
                path: relative.clone(),
            });
        }
    }

    let package = SkillPackage::load(&package_root).map_err(|_| PluginError::PackageLoad)?;
    let lint = package.lint(false);
    if !lint.errors.is_empty() {
        return Err(PluginError::LintFailed {
            errors: lint.errors.len(),
        });
    }
    Ok(())
}

pub fn run_plugin(
    plugin_program: &Path,
    request: &ExportRequest,
) -> Result<PluginRun, PluginError> {
    validate_request(request)?;
    let input_root = Path::new(&request.input_root);
    let output_root = Path::new(&request.output_root);
    validate_process_root(input_root)?;
    prepare_output_root(output_root)?;

    let mut request_file = NamedTempFile::new().map_err(PluginError::RequestIo)?;
    let request_json = serde_json::to_vec_pretty(request).map_err(|error| {
        PluginError::RequestIo(io::Error::new(io::ErrorKind::InvalidData, error))
    })?;
    request_file
        .write_all(&request_json)
        .map_err(PluginError::RequestIo)?;
    request_file.flush().map_err(PluginError::RequestIo)?;

    let output = Command::new(plugin_program)
        .args(["--input", request_file.path().to_string_lossy().as_ref()])
        .args(["--output", output_root.to_string_lossy().as_ref()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .map_err(PluginError::Spawn)?;

    let diagnostics = String::from_utf8_lossy(&output.stderr).into_owned();
    match output.status.code() {
        Some(0) => {
            let manifest = serde_json::from_slice::<PluginExportManifest>(&output.stdout)?;
            validate_plugin_manifest(request, &manifest, output_root)?;
            Ok(PluginRun {
                manifest,
                diagnostics,
            })
        }
        Some(PLUGIN_INVALID_INPUT_EXIT_CODE) => Err(PluginError::PluginInvalidInput),
        Some(PLUGIN_POLICY_FAILURE_EXIT_CODE) => Err(PluginError::PluginPolicyFailure),
        code => Err(PluginError::PluginCrashed { code }),
    }
}

fn canonical_output_root(path: &Path) -> Result<PathBuf, PluginError> {
    validate_process_root(path)?;
    path.canonicalize()
        .map_err(|_| PluginError::UnsafeOutputRoot {
            path: path.to_owned(),
        })
}

fn validate_process_root(path: &Path) -> Result<(), PluginError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| PluginError::UnsafeOutputRoot {
        path: path.to_owned(),
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PluginError::UnsafeOutputRoot {
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn prepare_output_root(path: &Path) -> Result<(), PluginError> {
    if !ancestors_are_safe(path) {
        return Err(PluginError::UnsafeOutputRoot {
            path: path.to_owned(),
        });
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(PluginError::UnsafeOutputRoot {
                path: path.to_owned(),
            })
        }
        Ok(_) => {
            let mut entries = fs::read_dir(path).map_err(|_| PluginError::UnsafeOutputRoot {
                path: path.to_owned(),
            })?;
            if entries.next().is_some() {
                return Err(PluginError::UnsafeOutputRoot {
                    path: path.to_owned(),
                });
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(|_| PluginError::UnsafeOutputRoot {
                path: path.to_owned(),
            })?;
            if ancestors_are_safe(path) {
                Ok(())
            } else {
                Err(PluginError::UnsafeOutputRoot {
                    path: path.to_owned(),
                })
            }
        }
        Err(_) => Err(PluginError::UnsafeOutputRoot {
            path: path.to_owned(),
        }),
    }
}

fn resolve_relative_directory(root: &Path, relative: &str) -> Result<PathBuf, PluginError> {
    if relative == "." {
        return Ok(root.to_owned());
    }
    let path = resolve_relative(root, relative)?;
    let metadata = fs::symlink_metadata(&path).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => PluginError::MissingManifestPath {
            path: relative.to_owned(),
        },
        _ => PluginError::UnsupportedManifestPath {
            path: relative.to_owned(),
        },
    })?;
    if metadata.file_type().is_symlink() {
        return Err(PluginError::SymlinkManifestPath {
            path: relative.to_owned(),
        });
    }
    if !metadata.is_dir() {
        return Err(PluginError::UnsupportedManifestPath {
            path: relative.to_owned(),
        });
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| PluginError::UnsafeManifestPath {
            path: relative.to_owned(),
        })?;
    if !canonical.starts_with(root) {
        return Err(PluginError::UnsafeManifestPath {
            path: relative.to_owned(),
        });
    }
    Ok(canonical)
}

fn resolve_relative_file(root: &Path, relative: &str) -> Result<PathBuf, PluginError> {
    let path = resolve_relative(root, relative)?;
    let metadata = fs::symlink_metadata(&path).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => PluginError::MissingManifestPath {
            path: relative.to_owned(),
        },
        _ => PluginError::UnsupportedManifestPath {
            path: relative.to_owned(),
        },
    })?;
    if metadata.file_type().is_symlink() {
        return Err(PluginError::SymlinkManifestPath {
            path: relative.to_owned(),
        });
    }
    if !metadata.is_file() {
        return Err(PluginError::UnsupportedManifestPath {
            path: relative.to_owned(),
        });
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| PluginError::UnsafeManifestPath {
            path: relative.to_owned(),
        })?;
    if !canonical.starts_with(root) {
        return Err(PluginError::UnsafeManifestPath {
            path: relative.to_owned(),
        });
    }
    Ok(canonical)
}

fn resolve_relative(root: &Path, relative: &str) -> Result<PathBuf, PluginError> {
    if relative.is_empty() || relative.contains('\\') || Path::new(relative).is_absolute() {
        return Err(PluginError::UnsafeManifestPath {
            path: relative.to_owned(),
        });
    }
    let mut path = root.to_owned();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(value) => path.push(value),
            Component::CurDir => {
                return Err(PluginError::UnsafeManifestPath {
                    path: relative.to_owned(),
                });
            }
            Component::RootDir | Component::ParentDir | Component::Prefix(_) => {
                return Err(PluginError::UnsafeManifestPath {
                    path: relative.to_owned(),
                });
            }
        }
    }
    Ok(path)
}

fn validate_capabilities(capabilities: &[String]) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for capability in capabilities {
        if capability.is_empty()
            || !capability.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
            })
            || !seen.insert(capability)
        {
            return Err(capability.clone());
        }
    }
    Ok(())
}

fn is_safe_target(target: &str) -> bool {
    target
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn is_clean_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                Component::RootDir | Component::Prefix(_) | Component::Normal(_)
            )
        })
}

fn ancestors_are_safe(path: &Path) -> bool {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata)
                if metadata.file_type().is_symlink() && !is_allowed_system_alias(&current) =>
            {
                return false;
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(_) => return false,
        }
    }
    true
}

fn is_allowed_system_alias(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        if path != Path::new("/etc") && path != Path::new("/tmp") && path != Path::new("/var") {
            return false;
        }
        path.canonicalize().is_ok_and(|canonical| {
            matches!(
                canonical.to_str(),
                Some("/private/etc") | Some("/private/tmp") | Some("/private/var")
            )
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        false
    }
}
