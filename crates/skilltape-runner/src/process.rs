use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::time::Duration;

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio_util::sync::CancellationToken;

/// The process invocation handed to an adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessRequest {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub timeout: Duration,
    pub max_output_bytes: usize,
}

/// A process can finish normally, exceed its deadline, or be cancelled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessStatus {
    Exited,
    TimedOut,
    Cancelled,
}

/// Bounded process output returned by an adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessOutput {
    pub status: ProcessStatus,
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

impl ProcessOutput {
    pub fn cancelled() -> Self {
        Self {
            status: ProcessStatus::Cancelled,
            exit_code: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }
}

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("failed to spawn process")]
    SpawnFailed,
    #[error("process I/O failed: {message}")]
    Io { message: String },
}

pub type ProcessFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ProcessOutput, ProcessError>> + Send + 'a>>;

/// Injectable process boundary used by the runner and its integration tests.
pub trait ProcessAdapter: Send + Sync {
    fn run<'a>(&'a self, request: ProcessRequest, cancel: CancellationToken) -> ProcessFuture<'a>;
}

/// Tokio-backed process adapter used by [`crate::run_skill`].
#[derive(Clone, Copy, Debug, Default)]
pub struct TokioProcessAdapter;

impl ProcessAdapter for TokioProcessAdapter {
    fn run<'a>(&'a self, request: ProcessRequest, cancel: CancellationToken) -> ProcessFuture<'a> {
        Box::pin(async move { run_process(request, cancel).await })
    }
}

async fn run_process(
    request: ProcessRequest,
    cancel: CancellationToken,
) -> Result<ProcessOutput, ProcessError> {
    let mut command = Command::new(&request.program);
    command
        .args(&request.args)
        .current_dir(&request.cwd)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        // Every invocation owns a process group so cancellation and timeout
        // cleanup also reaches descendants that inherit the stdio pipes.
        command.as_std_mut().process_group(0);
    }

    let mut child = command.spawn().map_err(|_| ProcessError::SpawnFailed)?;
    let stdout = child.stdout.take().ok_or_else(|| ProcessError::Io {
        message: "spawned process did not provide stdout".into(),
    })?;
    let stderr = child.stderr.take().ok_or_else(|| ProcessError::Io {
        message: "spawned process did not provide stderr".into(),
    })?;

    let stdout_task = tokio::spawn(read_bounded(stdout, request.max_output_bytes));
    let stderr_task = tokio::spawn(read_bounded(stderr, request.max_output_bytes));

    let (status, exit_code) = tokio::select! {
        result = child.wait() => {
            let status = result.map_err(|error| ProcessError::Io {
                message: error.to_string(),
            })?;
            (ProcessStatus::Exited, status.code())
        }
        _ = tokio::time::sleep(request.timeout) => {
            let exit_code = terminate_child(&mut child).await?;
            (ProcessStatus::TimedOut, exit_code)
        }
        _ = cancel.cancelled() => {
            let exit_code = terminate_child(&mut child).await?;
            (ProcessStatus::Cancelled, exit_code)
        }
    };

    let (stdout, stdout_truncated) = join_reader(stdout_task).await?;
    let (stderr, stderr_truncated) = join_reader(stderr_task).await?;

    Ok(ProcessOutput {
        status,
        exit_code,
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    })
}

async fn read_bounded<R>(
    reader: R,
    max_output_bytes: usize,
) -> Result<(Vec<u8>, bool), ProcessError>
where
    R: AsyncRead + Unpin,
{
    let mut reader = reader;
    let mut output = Vec::with_capacity(max_output_bytes.min(8192));
    let mut buffer = [0_u8; 8192];
    let mut truncated = false;
    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .await
            .map_err(|error| ProcessError::Io {
                message: error.to_string(),
            })?;
        if bytes_read == 0 {
            break;
        }

        let remaining = max_output_bytes.saturating_sub(output.len());
        let retained = bytes_read.min(remaining);
        output.extend_from_slice(&buffer[..retained]);
        if retained < bytes_read {
            truncated = true;
        }
    }
    Ok((output, truncated))
}

async fn join_reader(
    task: tokio::task::JoinHandle<Result<(Vec<u8>, bool), ProcessError>>,
) -> Result<(Vec<u8>, bool), ProcessError> {
    task.await.map_err(|error| ProcessError::Io {
        message: error.to_string(),
    })?
}

async fn terminate_child(child: &mut Child) -> Result<Option<i32>, ProcessError> {
    #[cfg(unix)]
    {
        if let Some(pid) = child.id() {
            signal_process_group(pid, libc::SIGTERM);
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = child.try_wait().map_err(|error| ProcessError::Io {
                message: error.to_string(),
            })?;
            // The parent may have exited while a descendant still owns the
            // process group and its stdio pipes, so always finish the group.
            signal_process_group(pid, libc::SIGKILL);
        }
    }

    #[cfg(not(unix))]
    {
        child.kill().await.map_err(|error| ProcessError::Io {
            message: error.to_string(),
        })?;
    }

    child
        .wait()
        .await
        .map(|status| status.code())
        .map_err(|error| ProcessError::Io {
            message: error.to_string(),
        })
}

#[cfg(unix)]
fn signal_process_group(pid: u32, signal: libc::c_int) {
    if let Ok(pid) = libc::pid_t::try_from(pid) {
        // A negative pid targets the process group created for the command.
        // The group may already have exited; cleanup is intentionally best effort.
        unsafe {
            let _ = libc::kill(-pid, signal);
        }
    }
}
