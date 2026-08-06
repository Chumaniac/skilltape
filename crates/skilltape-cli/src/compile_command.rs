use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use serde::Serialize;
use serde_json::Value;
use skilltape_compiler::{
    CompileError, CompileRequest, CompileTarget, Compiler, DeterministicCompiler, TapeSession,
};
use skilltape_core::SkillPackage;
use skilltape_tape::{TapeStore, TapeStoreError};
use tempfile::{Builder, TempDir};
use thiserror::Error;

const INPUT_ERROR_EXIT_CODE: u8 = 2;
const POLICY_ERROR_EXIT_CODE: u8 = 3;
const TARGET_NAME: &str = "generic-agent-skill";
const TARGET_VERSION: &str = "0.1.0";

#[derive(Debug)]
pub(crate) struct CompileConfig {
    pub tape: PathBuf,
    pub output: PathBuf,
    pub provider: Option<String>,
    pub accept_proposal: bool,
}

#[derive(Debug, Error)]
enum CompileCommandError {
    #[error(
        "compile provider `{provider}` is unavailable offline; no provider registry is configured"
    )]
    ProviderOffline { provider: String },
    #[error("accepting a proposal requires an explicit provider")]
    ProposalProviderRequired,
    #[error("compile output already exists: {0}")]
    OutputExists(PathBuf),
    #[error("compile output path is unsafe: {0}")]
    UnsafeOutput(PathBuf),
    #[error("compile output name is not a valid text path component")]
    InvalidOutputName,
    #[error("compile tape failed: {0}")]
    Tape(#[from] TapeStoreError),
    #[error("compile failed: {0}")]
    Compiler(#[from] CompileError),
    #[error("compile materialization failed: {0}")]
    Io(#[from] io::Error),
    #[error("compile package serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("compiled package failed to load: {0}")]
    Package(#[from] skilltape_core::PackageError),
    #[error("compiled package failed lint with {errors} error(s)")]
    Lint { errors: usize },
    #[error("compiler fixture path is unsafe: {0}")]
    UnsafeFixturePath(String),
}

impl CompileCommandError {
    fn exit_code(&self) -> ExitCode {
        match self {
            Self::ProviderOffline { .. } | Self::ProposalProviderRequired => {
                ExitCode::from(POLICY_ERROR_EXIT_CODE)
            }
            _ => ExitCode::from(INPUT_ERROR_EXIT_CODE),
        }
    }
}

pub(crate) fn run(config: CompileConfig) -> ExitCode {
    let output = config.output.clone();
    match compile(config) {
        Ok(()) => {
            println!("Compiled skill at {}", output.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            error.exit_code()
        }
    }
}

fn compile(config: CompileConfig) -> Result<(), CompileCommandError> {
    if let Some(provider) = config.provider {
        return Err(CompileCommandError::ProviderOffline { provider });
    }
    if config.accept_proposal {
        return Err(CompileCommandError::ProposalProviderRequired);
    }

    validate_output_path(&config.output)?;
    ensure_output_available(&config.output)?;
    let name = compile_name(&config.output)?;
    let target = CompileTarget::new(TARGET_NAME, TARGET_VERSION)?;

    let store = TapeStore::open(&config.tape)?;
    let events = store
        .read_events()?
        .collect::<Result<Vec<_>, TapeStoreError>>()?;
    let tape = TapeSession::new(events)?;
    let request = CompileRequest::new(tape, name, target.clone())?;
    let artifact = DeterministicCompiler.compile(request)?;

    let parent = output_parent(&config.output);
    fs::create_dir_all(parent)?;
    let staging = Builder::new()
        .prefix(".skilltape-compile-")
        .tempdir_in(parent)?;
    materialize(&staging, &artifact, &target)?;
    lint_staged_package(&staging)?;

    ensure_output_available(&config.output)?;
    publish(staging, &config.output)
}

fn validate_output_path(output: &Path) -> Result<(), CompileCommandError> {
    if output
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(CompileCommandError::UnsafeOutput(output.to_owned()));
    }

    let Some(file_name) = output.file_name() else {
        return Err(CompileCommandError::UnsafeOutput(output.to_owned()));
    };
    if file_name == "." || file_name == ".." {
        return Err(CompileCommandError::UnsafeOutput(output.to_owned()));
    }
    Ok(())
}

fn ensure_output_available(output: &Path) -> Result<(), CompileCommandError> {
    match fs::symlink_metadata(output) {
        Ok(_) => Err(CompileCommandError::OutputExists(output.to_owned())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CompileCommandError::Io(error)),
    }
}

fn compile_name(output: &Path) -> Result<String, CompileCommandError> {
    let raw = output
        .file_name()
        .ok_or(CompileCommandError::InvalidOutputName)?
        .to_string_lossy();
    let mut name = String::new();
    let mut separator = false;
    for character in raw.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
            name.push(character);
            separator = false;
        } else if !separator {
            name.push('-');
            separator = true;
        }
    }

    let name = name.trim_matches('-').to_owned();
    if name.is_empty() {
        Ok("compiled-skill".to_owned())
    } else {
        Ok(name)
    }
}

fn output_parent(output: &Path) -> &Path {
    output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn materialize(
    staging: &TempDir,
    artifact: &skilltape_compiler::CompileOutput,
    target: &CompileTarget,
) -> Result<(), CompileCommandError> {
    for (path, contents) in &artifact.fixtures.files {
        write_staged_file(staging.path(), path, contents)?;
    }
    write_staged_file(staging.path(), "SKILL.md", &artifact.skill_markdown)?;
    write_staged_file(
        staging.path(),
        "workflow.yaml",
        &json_document(&artifact.workflow)?,
    )?;
    write_staged_file(
        staging.path(),
        "permissions.json",
        &json_document(&artifact.permissions)?,
    )?;
    write_staged_file(
        staging.path(),
        "compile.json",
        &json_document(&artifact.provenance_document(target.clone()))?,
    )?;
    Ok(())
}

fn write_staged_file(
    root: &Path,
    relative: &str,
    contents: &str,
) -> Result<(), CompileCommandError> {
    let path = safe_fixture_path(root, relative)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    Ok(())
}

fn safe_fixture_path(root: &Path, relative: &str) -> Result<PathBuf, CompileCommandError> {
    let relative_path = Path::new(relative);
    if relative_path.as_os_str().is_empty()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(CompileCommandError::UnsafeFixturePath(relative.to_owned()));
    }
    Ok(root.join(relative_path))
}

fn json_document<T: Serialize>(value: &T) -> Result<String, CompileCommandError> {
    let mut document = serde_json::to_value(value)?;
    remove_null_members(&mut document);
    Ok(format!("{}\n", serde_json::to_string_pretty(&document)?))
}

fn remove_null_members(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.retain(|_, value| {
                remove_null_members(value);
                !value.is_null()
            });
        }
        Value::Array(array) => {
            for value in array {
                remove_null_members(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn lint_staged_package(staging: &TempDir) -> Result<(), CompileCommandError> {
    let package = SkillPackage::load(staging.path())?;
    let report = package.lint(false);
    if report.errors.is_empty() {
        Ok(())
    } else {
        Err(CompileCommandError::Lint {
            errors: report.errors.len(),
        })
    }
}

fn publish(staging: TempDir, output: &Path) -> Result<(), CompileCommandError> {
    fs::rename(staging.path(), output)?;
    Ok(())
}
