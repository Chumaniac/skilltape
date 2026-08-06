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
    #[error("no supported process sandbox is available")]
    SandboxUnavailable,
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
    let mut command = sandboxed_command(&request)?;
    command
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

    let (stdout, stdout_truncated, stderr, stderr_truncated) =
        match join_readers(stdout_task, stderr_task).await {
            Ok((stdout, stdout_truncated, stderr, stderr_truncated)) => {
                (stdout, stdout_truncated, stderr, stderr_truncated)
            }
            Err(error) if matches!(status, ProcessStatus::TimedOut | ProcessStatus::Cancelled) => {
                (Vec::new(), true, error.to_string().into_bytes(), true)
            }
            Err(error) => return Err(error),
        };

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

async fn join_readers(
    mut stdout_task: tokio::task::JoinHandle<Result<(Vec<u8>, bool), ProcessError>>,
    mut stderr_task: tokio::task::JoinHandle<Result<(Vec<u8>, bool), ProcessError>>,
) -> Result<(Vec<u8>, bool, Vec<u8>, bool), ProcessError> {
    let joined = async {
        let (stdout, stdout_truncated) = join_reader(&mut stdout_task).await?;
        let (stderr, stderr_truncated) = join_reader(&mut stderr_task).await?;
        Ok::<_, ProcessError>((stdout, stdout_truncated, stderr, stderr_truncated))
    };
    match tokio::time::timeout(Duration::from_millis(500), joined).await {
        Ok(result) => result,
        Err(_) => {
            stdout_task.abort();
            stderr_task.abort();
            Err(ProcessError::Io {
                message: "process output readers did not close before the cleanup deadline".into(),
            })
        }
    }
}

async fn join_reader(
    task: &mut tokio::task::JoinHandle<Result<(Vec<u8>, bool), ProcessError>>,
) -> Result<(Vec<u8>, bool), ProcessError> {
    task.await.map_err(|error| ProcessError::Io {
        message: error.to_string(),
    })?
}

fn sandboxed_command(request: &ProcessRequest) -> Result<Command, ProcessError> {
    #[cfg(target_os = "macos")]
    {
        return macos_sandbox_command(request);
    }

    #[cfg(target_os = "linux")]
    {
        return linux_sandbox_command(request);
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = request;
        Err(ProcessError::SandboxUnavailable)
    }
}

#[cfg(target_os = "macos")]
fn macos_sandbox_command(request: &ProcessRequest) -> Result<Command, ProcessError> {
    const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
    if !std::path::Path::new(SANDBOX_EXEC).is_file() {
        return Err(ProcessError::SandboxUnavailable);
    }

    let canonical_workspace = request.cwd.canonicalize().map_err(|_| ProcessError::Io {
        message: "sandbox workspace is unavailable".into(),
    })?;
    let workspace = profile_path(canonical_workspace.to_string_lossy().as_ref());
    let profile = format!(
        "(version 1)\
         (import \"system.sb\")\
         (allow process-exec)\
         (deny network*)\
         (deny file-read* (subpath \"/Users\"))\
         (deny file-read* (subpath \"/tmp\"))\
         (deny file-read* (subpath \"/private/tmp\"))\
         (deny file-read* (subpath \"/private/var/root\"))\
         (deny file-write* (subpath \"/Users\"))\
         (deny file-write* (subpath \"/tmp\"))\
         (deny file-write* (subpath \"/private/tmp\"))\
         (deny file-read* (subpath \"/private/var/folders\"))\
         (deny file-write* (subpath \"/private/var/folders\"))\
         (allow file-read* (subpath \"{workspace}\"))\
         (allow file-write* (subpath \"{workspace}\"))"
    );
    let mut command = Command::new(SANDBOX_EXEC);
    command
        .arg("-p")
        .arg(profile)
        .arg(&request.program)
        .args(&request.args);
    Ok(command)
}

#[cfg(target_os = "linux")]
fn linux_sandbox_command(request: &ProcessRequest) -> Result<Command, ProcessError> {
    let sandbox_available = std::process::Command::new("bwrap")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if !sandbox_available {
        return Err(ProcessError::SandboxUnavailable);
    }

    let program = if request.program == request.cwd.to_string_lossy()
        || request
            .program
            .starts_with(&format!("{}/", request.cwd.display()))
    {
        format!(
            "/workspace{}",
            &request.program[request.cwd.as_os_str().len()..]
        )
    } else {
        request.program.clone()
    };

    let mut command = Command::new("bwrap");
    command
        .args([
            "--die-with-parent",
            "--new-session",
            "--unshare-all",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--tmpfs",
            "/tmp",
            "--clearenv",
            "--setenv",
            "PATH",
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        ])
        .arg("--ro-bind")
        .args(["/usr", "/usr"])
        .arg("--ro-bind")
        .args(["/bin", "/bin"])
        .arg("--ro-bind")
        .args(["/lib", "/lib"])
        .arg("--ro-bind")
        .args(["/lib64", "/lib64"])
        .arg("--ro-bind")
        .args(["/etc", "/etc"])
        .arg("--bind")
        .args([request.cwd.to_string_lossy().as_ref(), "/workspace"])
        .arg("--chdir")
        .arg("/workspace")
        .arg("--")
        .arg(program)
        .args(&request.args);
    Ok(command)
}

fn profile_path(path: &str) -> String {
    path.replace('\\', "\\\\").replace('"', "\\\"")
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
