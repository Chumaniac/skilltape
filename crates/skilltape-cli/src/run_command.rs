use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};
use skilltape_core::{LoadedSkillPackage, PackageError, SkillPackage};
use skilltape_policy::PolicyEngine;
use skilltape_runner::{
    run_skill, PolicyDecisionRecord, PolicyPhase, ResourceLimits, RunError, RunFailure, RunRequest,
    RunStatus, RunSummary, StepStatus,
};
use skilltape_verify::{verify_run, Receipt, ReceiptStatus, VerifyError, VerifyRequest};
use tempfile::{Builder, TempDir};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const REPLAY_SCHEMA_V1: &str = "skilltape.dev/replay/v1";
const DEFAULT_MAX_OUTPUT_BYTES: usize = 64 * 1024;
const INPUT_ERROR_EXIT_CODE: u8 = 2;
const POLICY_ERROR_EXIT_CODE: u8 = 3;
const RUNTIME_ERROR_EXIT_CODE: u8 = 4;
const CANCELLED_EXIT_CODE: u8 = 5;

#[derive(Debug)]
pub(crate) struct ReplayConfig {
    pub skill_path: PathBuf,
    pub input: Option<PathBuf>,
    pub json: bool,
}

#[derive(Debug)]
pub(crate) struct VerifyConfig {
    pub skill_path: PathBuf,
    pub input: Option<PathBuf>,
    pub receipt: Option<PathBuf>,
    pub json: bool,
}

#[derive(Debug, Error)]
enum RunCommandError {
    #[error("skill package failed to load: {0}")]
    Package(#[from] PackageError),
    #[error("input root is invalid: {0}")]
    InvalidInput(PathBuf),
    #[error("temporary run workspace failed: {0}")]
    Temp(#[from] io::Error),
    #[error("runner failed: {0}")]
    Runner(#[from] RunError),
    #[error("verification failed: {0}")]
    Verify(#[from] VerifyError),
    #[error("receipt output path is unsafe: {0}")]
    UnsafeReceipt(PathBuf),
    #[error("receipt output already exists: {0}")]
    ReceiptExists(PathBuf),
    #[error("receipt output failed at {path}: {source}")]
    ReceiptIo {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("receipt serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("replay task failed: {0}")]
    Task(String),
}

struct RunRoots {
    input: PathBuf,
    _input_temp: Option<TempDir>,
    _output_parent: TempDir,
    output: PathBuf,
}

#[derive(Serialize)]
struct ReplayDocument {
    schema: &'static str,
    status: &'static str,
    steps: Vec<ReplayStep>,
    policy_decisions: Vec<ReplayPolicyDecision>,
    output_truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<ReplayFailure>,
}

#[derive(Serialize)]
struct ReplayStep {
    step_id: String,
    status: &'static str,
    exit_code: Option<i32>,
    stdout_sha256: String,
    stdout_bytes: usize,
    stdout_truncated: bool,
    stderr_sha256: String,
    stderr_bytes: usize,
    stderr_truncated: bool,
}

#[derive(Serialize)]
struct ReplayPolicyDecision {
    step_id: String,
    phase: &'static str,
    allowed: bool,
    code: String,
    reason: String,
    risk: String,
}

#[derive(Serialize)]
struct ReplayFailure {
    kind: &'static str,
    step_id: String,
    status: Option<&'static str>,
}

pub(crate) fn replay(config: ReplayConfig) -> ExitCode {
    let json = config.json;
    match execute_replay(config) {
        Ok(summary) => {
            if json {
                match serde_json::to_string(&replay_document(&summary)) {
                    Ok(document) => println!("{document}"),
                    Err(error) => {
                        eprintln!("replay output failed: {error}");
                        return ExitCode::from(RUNTIME_ERROR_EXIT_CODE);
                    }
                }
            } else {
                println!("{}", human_replay_summary(&summary));
            }
            replay_exit_code(&summary)
        }
        Err(error) => {
            eprintln!("{error}");
            error_exit_code(&error)
        }
    }
}

pub(crate) fn verify(config: VerifyConfig) -> ExitCode {
    let json = config.json;
    let receipt_path = config.receipt.clone();
    match execute_verify(config) {
        Ok(receipt) => {
            let document = match serde_json::to_string(&receipt) {
                Ok(document) => document,
                Err(error) => {
                    eprintln!("verification output failed: {error}");
                    return ExitCode::from(RUNTIME_ERROR_EXIT_CODE);
                }
            };
            if let Some(path) = receipt_path {
                if let Err(error) = write_receipt(&path, &document) {
                    eprintln!("{error}");
                    return error_exit_code(&error);
                }
            }
            if json {
                println!("{document}");
            } else {
                println!("{}", human_receipt_summary(&receipt));
            }
            verify_exit_code(receipt.status)
        }
        Err(error) => {
            eprintln!("{error}");
            error_exit_code(&error)
        }
    }
}

fn execute_replay(config: ReplayConfig) -> Result<RunSummary, RunCommandError> {
    let package = SkillPackage::load(config.skill_path)?;
    let roots = prepare_roots(config.input)?;
    let limits = limits_for(&package);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| RunCommandError::Task(error.to_string()))?;
    runtime.block_on(run_package(
        package,
        roots.input.clone(),
        roots.output.clone(),
        limits,
    ))
}

fn execute_verify(config: VerifyConfig) -> Result<Receipt, RunCommandError> {
    let package = SkillPackage::load(config.skill_path)?;
    let roots = prepare_roots(config.input)?;
    let limits = limits_for(&package);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| RunCommandError::Task(error.to_string()))?;
    runtime
        .block_on(verify_run(VerifyRequest {
            package,
            input_root: roots.input,
            output_root: roots.output,
            limits,
            assertions: Vec::new(),
        }))
        .map_err(RunCommandError::from)
}

async fn run_package(
    package: LoadedSkillPackage,
    input_root: PathBuf,
    output_root: PathBuf,
    limits: ResourceLimits,
) -> Result<RunSummary, RunCommandError> {
    let (sender, mut receiver) = mpsc::channel(64);
    let task = tokio::spawn(run_skill(
        RunRequest {
            package,
            input_root,
            output_root,
            limits,
        },
        PolicyEngine::default(),
        sender,
        CancellationToken::new(),
    ));
    while receiver.recv().await.is_some() {}
    task.await
        .map_err(|error| RunCommandError::Task(error.to_string()))?
        .map_err(RunCommandError::from)
}

fn prepare_roots(input: Option<PathBuf>) -> Result<RunRoots, RunCommandError> {
    let (input, input_temp) = match input {
        Some(path) => {
            ensure_input_directory(&path)?;
            (path, None)
        }
        None => {
            let temp = TempDir::new()?;
            (temp.path().to_owned(), Some(temp))
        }
    };
    let output_parent = TempDir::new()?;
    let output = output_parent.path().join("outputs");
    Ok(RunRoots {
        input,
        _input_temp: input_temp,
        _output_parent: output_parent,
        output,
    })
}

fn ensure_input_directory(path: &Path) -> Result<(), RunCommandError> {
    let absolute =
        absolute_path(path).map_err(|_| RunCommandError::InvalidInput(path.to_owned()))?;
    if !ancestors_are_safe(&absolute) {
        return Err(RunCommandError::InvalidInput(path.to_owned()));
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(RunCommandError::InvalidInput(path.to_owned()))
        }
        Ok(_) => Ok(()),
        Err(_) => Err(RunCommandError::InvalidInput(path.to_owned())),
    }
}

fn limits_for(package: &LoadedSkillPackage) -> ResourceLimits {
    ResourceLimits {
        max_processes: package.permissions.process.max_processes,
        step_timeout: Duration::from_millis(package.permissions.process.default_timeout_ms),
        max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
    }
}

fn replay_document(summary: &RunSummary) -> ReplayDocument {
    ReplayDocument {
        schema: REPLAY_SCHEMA_V1,
        status: run_status(summary.status),
        steps: summary
            .steps
            .iter()
            .map(|step| ReplayStep {
                step_id: step.step_id.clone(),
                status: step_status(step.status),
                exit_code: step.exit_code,
                stdout_sha256: digest(&step.stdout),
                stdout_bytes: step.stdout.len(),
                stdout_truncated: step.stdout_truncated,
                stderr_sha256: digest(&step.stderr),
                stderr_bytes: step.stderr.len(),
                stderr_truncated: step.stderr_truncated,
            })
            .collect(),
        policy_decisions: summary
            .policy_decisions
            .iter()
            .map(replay_policy_decision)
            .collect(),
        output_truncated: summary.output_truncated,
        failure: summary.failure.as_ref().map(replay_failure),
    }
}

fn replay_policy_decision(decision: &PolicyDecisionRecord) -> ReplayPolicyDecision {
    ReplayPolicyDecision {
        step_id: decision.step_id.clone(),
        phase: match decision.phase {
            PolicyPhase::Before => "before",
            PolicyPhase::After => "after",
        },
        allowed: decision.decision.allowed,
        code: decision.decision.code.clone(),
        reason: decision.decision.reason.clone(),
        risk: decision.decision.risk.as_str().to_owned(),
    }
}

fn replay_failure(failure: &RunFailure) -> ReplayFailure {
    match failure {
        RunFailure::PolicyDenied { step_id, .. } => ReplayFailure {
            kind: "policy_denied",
            step_id: step_id.clone(),
            status: None,
        },
        RunFailure::Step {
            step_id, status, ..
        } => ReplayFailure {
            kind: "step_failed",
            step_id: step_id.clone(),
            status: Some(step_status(*status)),
        },
    }
}

fn human_replay_summary(summary: &RunSummary) -> String {
    match summary.status {
        RunStatus::Succeeded => format!("Replay succeeded ({} step(s))", summary.steps.len()),
        RunStatus::Cancelled => "Replay cancelled".to_owned(),
        RunStatus::Failed => match summary.failure.as_ref() {
            Some(RunFailure::PolicyDenied { step_id, .. }) => {
                format!("Replay blocked by policy at step {step_id}")
            }
            Some(RunFailure::Step {
                step_id, status, ..
            }) => {
                format!("Replay failed at step {step_id} ({})", step_status(*status))
            }
            None => "Replay failed".to_owned(),
        },
    }
}

fn human_receipt_summary(receipt: &Receipt) -> String {
    match receipt.status {
        ReceiptStatus::Succeeded => "Verification succeeded".to_owned(),
        ReceiptStatus::RunFailed => "Verification failed: run_failed".to_owned(),
        ReceiptStatus::Cancelled => "Verification failed: cancelled".to_owned(),
        ReceiptStatus::AssertionFailed => "Verification failed: assertion_failed".to_owned(),
    }
}

fn replay_exit_code(summary: &RunSummary) -> ExitCode {
    match summary.status {
        RunStatus::Succeeded => ExitCode::SUCCESS,
        RunStatus::Cancelled => ExitCode::from(CANCELLED_EXIT_CODE),
        RunStatus::Failed => {
            if matches!(summary.failure, Some(RunFailure::PolicyDenied { .. })) {
                ExitCode::from(POLICY_ERROR_EXIT_CODE)
            } else {
                ExitCode::from(RUNTIME_ERROR_EXIT_CODE)
            }
        }
    }
}

fn verify_exit_code(status: ReceiptStatus) -> ExitCode {
    if status == ReceiptStatus::Succeeded {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(POLICY_ERROR_EXIT_CODE)
    }
}

fn error_exit_code(error: &RunCommandError) -> ExitCode {
    let code = match error {
        RunCommandError::Package(_)
        | RunCommandError::InvalidInput(_)
        | RunCommandError::UnsafeReceipt(_)
        | RunCommandError::ReceiptExists(_)
        | RunCommandError::ReceiptIo { .. } => INPUT_ERROR_EXIT_CODE,
        RunCommandError::Verify(error) => verify_error_code(error),
        RunCommandError::Runner(error) => runner_error_code(error),
        RunCommandError::Temp(_) | RunCommandError::Serialization(_) | RunCommandError::Task(_) => {
            RUNTIME_ERROR_EXIT_CODE
        }
    };
    ExitCode::from(code)
}

fn verify_error_code(error: &VerifyError) -> u8 {
    match error {
        VerifyError::InvalidInputRoot
        | VerifyError::InvalidAssertion { .. }
        | VerifyError::AssertionInput { .. }
        | VerifyError::Serialization(_)
        | VerifyError::Io(_) => INPUT_ERROR_EXIT_CODE,
        VerifyError::Runner(error) => runner_error_code(error),
        VerifyError::ReceiptSchema { .. } => RUNTIME_ERROR_EXIT_CODE,
    }
}

fn runner_error_code(error: &RunError) -> u8 {
    match error {
        RunError::InvalidInputRoot { .. }
        | RunError::InvalidLimits { .. }
        | RunError::UnsafeOutputRoot { .. } => INPUT_ERROR_EXIT_CODE,
        RunError::Workspace { .. }
        | RunError::Materialization { .. }
        | RunError::EventChannelClosed => RUNTIME_ERROR_EXIT_CODE,
    }
}

fn write_receipt(path: &Path, document: &str) -> Result<(), RunCommandError> {
    let path = absolute_path(path).map_err(|_| RunCommandError::UnsafeReceipt(path.to_owned()))?;
    ensure_safe_ancestors(&path)?;
    match fs::symlink_metadata(&path) {
        Ok(_) => return Err(RunCommandError::ReceiptExists(path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(source) => return Err(RunCommandError::ReceiptIo { path, source }),
    }

    let parent = path
        .parent()
        .ok_or_else(|| RunCommandError::UnsafeReceipt(path.clone()))?;
    fs::create_dir_all(parent).map_err(|source| RunCommandError::ReceiptIo {
        path: parent.to_owned(),
        source,
    })?;
    ensure_safe_ancestors(&path)?;
    match fs::symlink_metadata(&path) {
        Ok(_) => return Err(RunCommandError::ReceiptExists(path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(source) => return Err(RunCommandError::ReceiptIo { path, source }),
    }

    let mut temporary = Builder::new()
        .prefix(".skilltape-receipt-")
        .tempfile_in(parent)
        .map_err(|source| RunCommandError::ReceiptIo {
            path: parent.to_owned(),
            source,
        })?;
    temporary
        .write_all(document.as_bytes())
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|source| RunCommandError::ReceiptIo {
            path: temporary.path().to_owned(),
            source,
        })?;
    match fs::hard_link(temporary.path(), &path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
            Err(RunCommandError::ReceiptExists(path))
        }
        Err(source) => Err(RunCommandError::ReceiptIo { path, source }),
    }
}

fn absolute_path(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn ensure_safe_ancestors(path: &Path) -> Result<(), RunCommandError> {
    if !ancestors_are_safe(path) {
        return Err(RunCommandError::UnsafeReceipt(path.to_owned()));
    }
    Ok(())
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

fn run_status(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Succeeded => "succeeded",
        RunStatus::Failed => "failed",
        RunStatus::Cancelled => "cancelled",
    }
}

fn step_status(status: StepStatus) -> &'static str {
    match status {
        StepStatus::RunStarted => "run_started",
        StepStatus::Started => "started",
        StepStatus::Succeeded => "succeeded",
        StepStatus::Failed => "failed",
        StepStatus::Denied => "denied",
        StepStatus::TimedOut => "timed_out",
        StepStatus::Cancelled => "cancelled",
        StepStatus::SpawnFailed => "spawn_failed",
        StepStatus::RunSucceeded => "run_succeeded",
        StepStatus::RunFailed => "run_failed",
        StepStatus::RunCancelled => "run_cancelled",
    }
}

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
