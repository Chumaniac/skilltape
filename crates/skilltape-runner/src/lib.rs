//! Guarded replay execution for loaded SkillTape packages.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use skilltape_core::LoadedSkillPackage;
use skilltape_policy::{FileAccess, PolicyDecision, PolicyEngine};
use skilltape_schema::Step;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

mod process;
mod workspace;

pub use process::{
    ProcessAdapter, ProcessError, ProcessFuture, ProcessOutput, ProcessRequest, ProcessStatus,
    TokioProcessAdapter,
};

use workspace::{copy_path, make_directory, move_path, ReplayWorkspace, WorkspaceError};

const RUN_STEP_ID: &str = "__run__";

/// The inputs and limits for one isolated replay.
pub struct RunRequest {
    pub package: LoadedSkillPackage,
    pub input_root: PathBuf,
    pub output_root: PathBuf,
    pub limits: ResourceLimits,
}

/// Resource ceilings applied to every process invocation.
///
/// Replay steps execute sequentially, so the active process count never exceeds
/// one. `max_processes` remains an explicit caller ceiling and must be at least
/// one; values above one are looser ceilings that the sequential runner can
/// satisfy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceLimits {
    pub max_processes: u32,
    pub step_timeout: Duration,
    pub max_output_bytes: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_processes: 1,
            step_timeout: Duration::from_secs(120),
            max_output_bytes: 1024 * 1024,
        }
    }
}

/// Stable statuses used by lifecycle events and step summaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepStatus {
    RunStarted,
    Started,
    Succeeded,
    Failed,
    Denied,
    TimedOut,
    Cancelled,
    SpawnFailed,
    RunSucceeded,
    RunFailed,
    RunCancelled,
}

/// One ordered lifecycle record. Sequence numbers start at zero for RunStarted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunEvent {
    pub sequence: u64,
    pub step_id: String,
    pub status: StepStatus,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunStatus {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyPhase {
    Before,
    After,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyDecisionRecord {
    pub step_id: String,
    pub phase: PolicyPhase,
    pub decision: PolicyDecision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepSummary {
    pub step_id: String,
    pub status: StepStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RunFailure {
    PolicyDenied {
        step_id: String,
        decision: PolicyDecision,
    },
    Step {
        step_id: String,
        status: StepStatus,
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunSummary {
    pub status: RunStatus,
    pub steps: Vec<StepSummary>,
    pub policy_decisions: Vec<PolicyDecisionRecord>,
    pub output_truncated: bool,
    pub failure: Option<RunFailure>,
}

#[derive(Debug, Error)]
pub enum RunError {
    #[error("input root is invalid: {path}")]
    InvalidInputRoot { path: PathBuf },
    #[error("resource limits are invalid: {message}")]
    InvalidLimits { message: String },
    #[error("output root overlaps an input or package path: {path}")]
    UnsafeOutputRoot { path: PathBuf },
    #[error("runner workspace setup failed: {message}")]
    Workspace { message: String },
    #[error("output materialization failed: {message}")]
    Materialization { message: String },
    #[error("run event channel closed")]
    EventChannelClosed,
}

/// Run a package with the real async process adapter.
pub async fn run_skill(
    request: RunRequest,
    policy: PolicyEngine,
    events: mpsc::Sender<RunEvent>,
    cancel: CancellationToken,
) -> Result<RunSummary, RunError> {
    let adapter = TokioProcessAdapter;
    run_skill_with_adapter(request, policy, events, cancel, &adapter).await
}

/// Run a package through an injectable process boundary.
pub async fn run_skill_with_adapter<A>(
    request: RunRequest,
    policy: PolicyEngine,
    events: mpsc::Sender<RunEvent>,
    cancel: CancellationToken,
    adapter: &A,
) -> Result<RunSummary, RunError>
where
    A: ProcessAdapter + ?Sized,
{
    validate_limits(&request.limits, &request.package.permissions)?;
    validate_output_root(
        &request.package.root,
        &request.input_root,
        &request.output_root,
    )?;

    let workspace = match ReplayWorkspace::prepare(&request.package, &request.input_root) {
        Ok(workspace) => workspace,
        Err(WorkspaceError::InvalidInputRoot { path }) => {
            return Err(RunError::InvalidInputRoot { path });
        }
        Err(error) => {
            return Err(RunError::Workspace {
                message: error.to_string(),
            })
        }
    };

    let mut summary = RunSummary {
        status: RunStatus::Succeeded,
        steps: Vec::new(),
        policy_decisions: Vec::new(),
        output_truncated: false,
        failure: None,
    };
    let mut sequence = 0;
    emit_event(
        &events,
        &mut sequence,
        RUN_STEP_ID,
        StepStatus::RunStarted,
        Vec::new(),
        Vec::new(),
        None,
    )
    .await?;

    for step in &request.package.workflow.steps {
        let step_id = step_id(step).to_owned();
        if cancel.is_cancelled() {
            summary.status = RunStatus::Cancelled;
            break;
        }

        emit_event(
            &events,
            &mut sequence,
            &step_id,
            StepStatus::Started,
            Vec::new(),
            Vec::new(),
            None,
        )
        .await?;

        let before = policy_checks(&policy, step, &request.package);
        record_decisions(
            &mut summary.policy_decisions,
            &step_id,
            PolicyPhase::Before,
            &before,
        );
        if let Some(denied) = before.iter().find(|decision| !decision.allowed) {
            let failure = RunFailure::PolicyDenied {
                step_id: step_id.clone(),
                decision: denied.clone(),
            };
            summary.failure = Some(failure);
            summary.status = RunStatus::Failed;
            summary.steps.push(StepSummary {
                step_id: step_id.clone(),
                status: StepStatus::Denied,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
            });
            emit_event(
                &events,
                &mut sequence,
                &step_id,
                StepStatus::Denied,
                Vec::new(),
                Vec::new(),
                None,
            )
            .await?;
            break;
        }

        let execution =
            execute_step(step, &workspace, &request.limits, adapter, cancel.clone()).await;
        let after = policy_checks(&policy, step, &request.package);
        record_decisions(
            &mut summary.policy_decisions,
            &step_id,
            PolicyPhase::After,
            &after,
        );

        summary.output_truncated |= execution.stdout_truncated || execution.stderr_truncated;
        summary.steps.push(StepSummary {
            step_id: step_id.clone(),
            status: execution.status,
            exit_code: execution.exit_code,
            stdout: bytes_to_string(&execution.stdout),
            stderr: bytes_to_string(&execution.stderr),
        });
        emit_event(
            &events,
            &mut sequence,
            &step_id,
            execution.status,
            execution.stdout,
            execution.stderr,
            execution.exit_code,
        )
        .await?;

        if let Some(failure) = execution.failure {
            summary.failure = Some(failure);
        }
        match execution.status {
            StepStatus::Succeeded => {}
            StepStatus::Cancelled => {
                summary.status = RunStatus::Cancelled;
                break;
            }
            _ => {
                summary.status = RunStatus::Failed;
                break;
            }
        }
    }

    if summary.status == RunStatus::Succeeded {
        let output_paths = declared_output_paths(&request.package);
        if let Err(error) = workspace.materialize_outputs(&request.output_root, &output_paths) {
            let message = error.to_string();
            summary.status = RunStatus::Failed;
            summary.failure = Some(RunFailure::Step {
                step_id: "__outputs__".into(),
                status: StepStatus::Failed,
                message: message.clone(),
            });
            emit_event(
                &events,
                &mut sequence,
                RUN_STEP_ID,
                StepStatus::RunFailed,
                Vec::new(),
                message.as_bytes().to_vec(),
                None,
            )
            .await?;
            return Err(RunError::Materialization { message });
        }
    }

    let final_status = match summary.status {
        RunStatus::Succeeded => StepStatus::RunSucceeded,
        RunStatus::Failed => StepStatus::RunFailed,
        RunStatus::Cancelled => StepStatus::RunCancelled,
    };
    emit_event(
        &events,
        &mut sequence,
        RUN_STEP_ID,
        final_status,
        Vec::new(),
        Vec::new(),
        None,
    )
    .await?;

    Ok(summary)
}

#[derive(Debug)]
struct StepExecution {
    status: StepStatus,
    exit_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_truncated: bool,
    stderr_truncated: bool,
    failure: Option<RunFailure>,
}

struct ProcessInvocation {
    step_id: String,
    program: String,
    args: Vec<String>,
    timeout_ms: u64,
}

async fn execute_step<A>(
    step: &Step,
    workspace: &ReplayWorkspace,
    limits: &ResourceLimits,
    adapter: &A,
    cancel: CancellationToken,
) -> StepExecution
where
    A: ProcessAdapter + ?Sized,
{
    match step {
        Step::Exec(step) => {
            execute_process(
                ProcessInvocation {
                    step_id: step.id.clone(),
                    program: step.program.clone(),
                    args: step.args.clone(),
                    timeout_ms: step.timeout_ms,
                },
                workspace,
                limits,
                adapter,
                cancel,
            )
            .await
        }
        Step::Script(step) => {
            let program = match workspace.resolve(&step.path) {
                Ok(path) => path.to_string_lossy().into_owned(),
                Err(error) => return failed_action(&step.id, error.to_string()),
            };
            execute_process(
                ProcessInvocation {
                    step_id: step.id.clone(),
                    program,
                    args: step.args.clone(),
                    timeout_ms: step.timeout_ms,
                },
                workspace,
                limits,
                adapter,
                cancel,
            )
            .await
        }
        Step::File(step) => execute_file(step, workspace),
        Step::Assert(step) => execute_assert(step, workspace),
    }
}

async fn execute_process<A>(
    invocation: ProcessInvocation,
    workspace: &ReplayWorkspace,
    limits: &ResourceLimits,
    adapter: &A,
    cancel: CancellationToken,
) -> StepExecution
where
    A: ProcessAdapter + ?Sized,
{
    let timeout = Duration::from_millis(invocation.timeout_ms).min(limits.step_timeout);
    let request = ProcessRequest {
        program: invocation.program,
        args: invocation.args,
        cwd: workspace.root().to_path_buf(),
        timeout,
        max_output_bytes: limits.max_output_bytes,
    };

    match adapter.run(request, cancel).await {
        Ok(output) => process_output(invocation.step_id, output, limits.max_output_bytes),
        Err(error) => StepExecution {
            status: StepStatus::SpawnFailed,
            exit_code: None,
            stdout: Vec::new(),
            stderr: error.to_string().into_bytes(),
            stdout_truncated: false,
            stderr_truncated: false,
            failure: Some(RunFailure::Step {
                step_id: invocation.step_id,
                status: StepStatus::SpawnFailed,
                message: error.to_string(),
            }),
        },
    }
}

fn process_output(step_id: String, output: ProcessOutput, limit: usize) -> StepExecution {
    let (stdout, stdout_truncated) = bound_output(output.stdout, limit);
    let (stderr, stderr_truncated) = bound_output(output.stderr, limit);
    let status = match output.status {
        ProcessStatus::Exited if output.exit_code == Some(0) => StepStatus::Succeeded,
        ProcessStatus::Exited => StepStatus::Failed,
        ProcessStatus::TimedOut => StepStatus::TimedOut,
        ProcessStatus::Cancelled => StepStatus::Cancelled,
    };
    let failure = (status != StepStatus::Succeeded).then(|| RunFailure::Step {
        step_id,
        status,
        message: match status {
            StepStatus::Failed => format!("process exited with code {:?}", output.exit_code),
            StepStatus::TimedOut => "process exceeded its timeout".into(),
            StepStatus::Cancelled => "process was cancelled".into(),
            _ => "process failed".into(),
        },
    });
    StepExecution {
        status,
        exit_code: output.exit_code,
        stdout,
        stderr,
        stdout_truncated: stdout_truncated || output.stdout_truncated,
        stderr_truncated: stderr_truncated || output.stderr_truncated,
        failure,
    }
}

fn execute_file(step: &skilltape_schema::FileStep, workspace: &ReplayWorkspace) -> StepExecution {
    let result = (|| {
        let to = workspace.resolve(&step.to_path)?;
        match step.operation.as_str() {
            "copy" => {
                let from = workspace.resolve(&step.from_path)?;
                copy_path(&from, &to)
            }
            "move" => {
                let from = workspace.resolve(&step.from_path)?;
                move_path(&from, &to)
            }
            "mkdir" => make_directory(&to),
            operation => Err(WorkspaceError::UnsafePath {
                path: format!("unsupported file operation `{operation}`"),
            }),
        }
    })();
    match result {
        Ok(()) => succeeded_action(),
        Err(error) => failed_action(&step.id, error.to_string()),
    }
}

fn execute_assert(
    step: &skilltape_schema::AssertStep,
    workspace: &ReplayWorkspace,
) -> StepExecution {
    let result = (|| {
        let relative = step
            .assertion
            .path
            .as_deref()
            .ok_or_else(|| "assertion requires a path".to_owned())?;
        let path = workspace
            .resolve(relative)
            .map_err(|error| error.to_string())?;
        workspace
            .ensure_safe_path(&path)
            .map_err(|error| error.to_string())?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.to_string()),
        };
        match step.assertion.assertion_type.as_str() {
            "file_exists" | "exists" => {
                if metadata.is_some() {
                    Ok(())
                } else {
                    Err(format!("expected `{relative}` to exist"))
                }
            }
            "file_absent" | "absent" => {
                if metadata.is_none() {
                    Ok(())
                } else {
                    Err(format!("expected `{relative}` to be absent"))
                }
            }
            "file_hash" | "hash" => {
                let expected = step
                    .assertion
                    .hash
                    .as_deref()
                    .ok_or_else(|| "file_hash assertion requires a hash".to_owned())?;
                let contents = fs::read(&path).map_err(|error| error.to_string())?;
                let actual = format!("{:x}", Sha256::digest(contents));
                if actual.eq_ignore_ascii_case(expected) {
                    Ok(())
                } else {
                    Err(format!(
                        "sha256 mismatch: expected {expected}, got {actual}"
                    ))
                }
            }
            assertion_type => Err(format!("unsupported assertion type `{assertion_type}`")),
        }
    })();
    match result {
        Ok(()) => succeeded_action(),
        Err(error) => failed_action(&step.id, error),
    }
}

fn succeeded_action() -> StepExecution {
    StepExecution {
        status: StepStatus::Succeeded,
        exit_code: None,
        stdout: Vec::new(),
        stderr: Vec::new(),
        stdout_truncated: false,
        stderr_truncated: false,
        failure: None,
    }
}

fn failed_action(step_id: &str, message: String) -> StepExecution {
    StepExecution {
        status: StepStatus::Failed,
        exit_code: None,
        stdout: Vec::new(),
        stderr: message.clone().into_bytes(),
        stdout_truncated: false,
        stderr_truncated: false,
        failure: Some(RunFailure::Step {
            step_id: step_id.to_owned(),
            status: StepStatus::Failed,
            message,
        }),
    }
}

fn policy_checks(
    policy: &PolicyEngine,
    step: &Step,
    package: &LoadedSkillPackage,
) -> Vec<PolicyDecision> {
    let mut decisions = Vec::new();
    let permissions = &package.permissions;
    match step {
        Step::Exec(step) => {
            decisions.push(policy.check_command(&step.program, &step.args, permissions));
            for output in &step.outputs {
                decisions.push(policy.check_path(&output.path, FileAccess::Write, permissions));
            }
        }
        Step::Script(step) => {
            decisions.push(policy.check_path(&step.path, FileAccess::Read, permissions));
            decisions.push(policy.check_command(&step.path, &step.args, permissions));
            for output in &step.outputs {
                decisions.push(policy.check_path(&output.path, FileAccess::Write, permissions));
            }
        }
        Step::File(step) => {
            if step.operation != "mkdir" {
                decisions.push(policy.check_path(&step.from_path, FileAccess::Read, permissions));
            }
            decisions.push(policy.check_path(&step.to_path, FileAccess::Write, permissions));
        }
        Step::Assert(step) => {
            decisions.push(policy.check_path(
                step.assertion.path.as_deref().unwrap_or_default(),
                FileAccess::Read,
                permissions,
            ));
        }
    }
    decisions
}

fn record_decisions(
    records: &mut Vec<PolicyDecisionRecord>,
    step_id: &str,
    phase: PolicyPhase,
    decisions: &[PolicyDecision],
) {
    records.extend(
        decisions
            .iter()
            .cloned()
            .map(|decision| PolicyDecisionRecord {
                step_id: step_id.to_owned(),
                phase,
                decision,
            }),
    );
}

fn declared_output_paths(package: &LoadedSkillPackage) -> Vec<String> {
    let mut paths = BTreeSet::new();
    paths.extend(
        package
            .manifest
            .outputs
            .iter()
            .map(|output| output.path.clone()),
    );
    for step in &package.workflow.steps {
        match step {
            Step::Exec(step) => paths.extend(step.outputs.iter().map(|output| output.path.clone())),
            Step::Script(step) => {
                paths.extend(step.outputs.iter().map(|output| output.path.clone()))
            }
            Step::File(step) => {
                paths.insert(step.to_path.clone());
            }
            Step::Assert(_) => {}
        }
    }
    paths.into_iter().collect()
}

fn step_id(step: &Step) -> &str {
    match step {
        Step::Exec(step) => &step.id,
        Step::Script(step) => &step.id,
        Step::File(step) => &step.id,
        Step::Assert(step) => &step.id,
    }
}

async fn emit_event(
    events: &mpsc::Sender<RunEvent>,
    sequence: &mut u64,
    step_id: &str,
    status: StepStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_code: Option<i32>,
) -> Result<(), RunError> {
    let event = RunEvent {
        sequence: *sequence,
        step_id: step_id.to_owned(),
        status,
        stdout: bytes_to_string(&stdout),
        stderr: bytes_to_string(&stderr),
        exit_code,
    };
    *sequence += 1;
    events
        .send(event)
        .await
        .map_err(|_| RunError::EventChannelClosed)
}

fn bound_output(mut output: Vec<u8>, limit: usize) -> (Vec<u8>, bool) {
    let truncated = output.len() > limit;
    if truncated {
        output.truncate(limit);
    }
    (output, truncated)
}

fn bytes_to_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

const MAX_ACTIVE_PROCESSES: u32 = 1;

fn validate_limits(
    limits: &ResourceLimits,
    permissions: &skilltape_schema::Permissions,
) -> Result<(), RunError> {
    if limits.max_processes < MAX_ACTIVE_PROCESSES {
        return Err(RunError::InvalidLimits {
            message: "resource max_processes must be at least one".into(),
        });
    }
    if permissions.process.max_processes < MAX_ACTIVE_PROCESSES {
        return Err(RunError::InvalidLimits {
            message: "package permissions process.max_processes must be at least one".into(),
        });
    }
    if limits.step_timeout.is_zero() {
        return Err(RunError::InvalidLimits {
            message: "step_timeout must be greater than zero".into(),
        });
    }
    Ok(())
}

fn validate_output_root(
    package_root: &Path,
    input_root: &Path,
    output_root: &Path,
) -> Result<(), RunError> {
    let input = comparable_path(input_root);
    let package = comparable_path(package_root);
    let output = comparable_path(output_root);
    if paths_overlap(&input, &output) || paths_overlap(&package, &output) {
        return Err(RunError::UnsafeOutputRoot {
            path: output_root.to_path_buf(),
        });
    }
    Ok(())
}

fn comparable_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    left == right || left.starts_with(right) || right.starts_with(left)
}
