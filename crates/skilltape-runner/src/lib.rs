//! Guarded replay execution for loaded SkillTape packages.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use skilltape_core::LoadedSkillPackage;
use skilltape_policy::{PolicyDecision, PolicyEngine};
use skilltape_schema::Step;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

mod execution;
mod input_io;
mod policy_checks;
mod process;
mod publication;
#[cfg(any(target_os = "windows", test))]
mod windows;
mod workspace;

pub use policy_checks::preview_policy;
pub use process::{
    ProcessAdapter, ProcessError, ProcessFuture, ProcessOutput, ProcessRequest, ProcessStatus,
    TokioProcessAdapter,
};
pub use publication::publish_directory_noreplace;

use execution::execute_step;
use policy_checks::{policy_checks, record_decisions};
use workspace::{ReplayWorkspace, WorkspaceError};

pub const RUNNER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Copy local inputs using the same bounded, no-link snapshot checks as Replay.
/// The destination must be new; this does not execute any task.
pub fn snapshot_inputs(source: &Path, destination: &Path) -> std::io::Result<()> {
    input_io::copy_input_tree(source, destination).map_err(std::io::Error::other)
}

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
    pub stdout_truncated: bool,
    pub stderr: String,
    pub stderr_truncated: bool,
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
    #[error("input capacity exceeded: {dimension} limit is {limit}")]
    InputCapacity { dimension: &'static str, limit: u64 },
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

/// Reject oversized or unsafe input trees before content hashing or workspace copying.
///
/// The metadata snapshot permits at most 10,000 descendant entries, depth 64,
/// 16 MiB per regular file and 64 MiB in total. It does not freeze the source,
/// enforce disk quotas, or validate domain semantics. Input hashing and staging
/// separately bound actual reads and recheck the inventory after completion.
pub fn preflight_input_capacity(input_root: &Path) -> Result<(), RunError> {
    workspace::validate_input_capacity(input_root).map_err(workspace_setup_error)
}

/// Hash a bounded input snapshot using the existing path/length/content digest format.
pub fn digest_input_tree(input_root: &Path) -> Result<String, RunError> {
    input_io::digest_input_tree(input_root).map_err(workspace_setup_error)
}

fn workspace_setup_error(error: WorkspaceError) -> RunError {
    match error {
        WorkspaceError::InvalidInputRoot { path } => RunError::InvalidInputRoot { path },
        WorkspaceError::InputCapacity { dimension, limit } => {
            RunError::InputCapacity { dimension, limit }
        }
        error => RunError::Workspace {
            message: error.to_string(),
        },
    }
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

    preflight_input_capacity(&request.input_root)?;
    let workspace = ReplayWorkspace::prepare(&request.package, &request.input_root)
        .map_err(workspace_setup_error)?;

    let mut summary = RunSummary {
        status: if cancel.is_cancelled() {
            RunStatus::Cancelled
        } else {
            RunStatus::Succeeded
        },
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
                stdout_truncated: false,
                stderr: String::new(),
                stderr_truncated: false,
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
            stdout_truncated: execution.stdout_truncated,
            stderr: bytes_to_string(&execution.stderr),
            stderr_truncated: execution.stderr_truncated,
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

/// Reject an output directory overlapping a package or input tree.
pub fn validate_output_root(
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
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let mut ancestor = absolute.as_path();
    let mut pending = Vec::new();
    loop {
        if let Ok(mut canonical) = ancestor.canonicalize() {
            for part in pending.iter().rev() {
                canonical.push(part);
            }
            return canonical;
        }
        match (ancestor.file_name(), ancestor.parent()) {
            (Some(part), Some(parent)) => {
                pending.push(part.to_owned());
                ancestor = parent;
            }
            _ => return absolute,
        }
    }
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    left == right || left.starts_with(right) || right.starts_with(left)
}
