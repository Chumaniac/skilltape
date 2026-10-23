//! One guarded step: process, file operation, or bounded assertion.

use std::fs;
use std::io::Read;
use std::time::Duration;

use sha2::{Digest, Sha256};
use skilltape_schema::Step;
use tokio_util::sync::CancellationToken;

use crate::workspace::{copy_path, make_directory, move_path, ReplayWorkspace, WorkspaceError};
use crate::{
    ProcessAdapter, ProcessOutput, ProcessRequest, ProcessStatus, ResourceLimits, RunFailure,
    StepStatus,
};

#[derive(Debug)]
pub(super) struct StepExecution {
    pub(super) status: StepStatus,
    pub(super) exit_code: Option<i32>,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) stdout_truncated: bool,
    pub(super) stderr_truncated: bool,
    pub(super) failure: Option<RunFailure>,
}

struct ProcessInvocation {
    step_id: String,
    program: String,
    args: Vec<String>,
    timeout_ms: u64,
}

pub(super) async fn execute_step<A>(
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
                let mut file = fs::File::open(&path).map_err(|error| error.to_string())?;
                let mut hasher = Sha256::new();
                let mut buffer = [0_u8; 8192];
                loop {
                    let bytes_read = match file.read(&mut buffer) {
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        result => result.map_err(|error| error.to_string())?,
                    };
                    if bytes_read == 0 {
                        break;
                    }
                    hasher.update(&buffer[..bytes_read]);
                }
                let actual = hasher
                    .finalize()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
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

fn bound_output(mut output: Vec<u8>, limit: usize) -> (Vec<u8>, bool) {
    let truncated = output.len() > limit;
    if truncated {
        output.truncate(limit);
    }
    (output, truncated)
}
