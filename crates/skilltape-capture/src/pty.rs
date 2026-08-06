use std::io::Read;
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, ExitStatus, PtySize};
use tokio_util::sync::CancellationToken;

use crate::{CaptureError, TerminalSize};

const POLL_INTERVAL: Duration = Duration::from_millis(10);
const TERMINATION_GRACE: Duration = Duration::from_millis(250);

pub(crate) struct PtyRequest {
    pub command: String,
    pub args: Vec<String>,
    pub workspace: std::path::PathBuf,
    pub output_limit: usize,
    pub terminal_size: TerminalSize,
    pub timeout: Duration,
}

pub(crate) struct PtyRunResult {
    pub exit_code: u32,
    pub signal: Option<String>,
    pub output: Vec<u8>,
    pub output_bytes: usize,
    pub output_truncated: bool,
    pub cancelled: bool,
    pub timed_out: bool,
    pub terminal_size: TerminalSize,
}

pub(crate) trait PtyAdapter: Send + Sync + 'static {
    fn run(
        &self,
        request: PtyRequest,
        cancel: CancellationToken,
    ) -> Result<PtyRunResult, CaptureError>;
}

pub(crate) struct PortablePtyAdapter;

impl PtyAdapter for PortablePtyAdapter {
    fn run(
        &self,
        request: PtyRequest,
        cancel: CancellationToken,
    ) -> Result<PtyRunResult, CaptureError> {
        let size = PtySize {
            rows: request.terminal_size.rows,
            cols: request.terminal_size.cols,
            pixel_width: request.terminal_size.pixel_width,
            pixel_height: request.terminal_size.pixel_height,
        };
        let pair = native_pty_system()
            .openpty(size)
            .map_err(|error| CaptureError::Pty(error.to_string()))?;
        let terminal_size = pair
            .master
            .get_size()
            .map(TerminalSize::from)
            .unwrap_or(request.terminal_size);
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| CaptureError::Pty(error.to_string()))?;

        let mut command = CommandBuilder::new(request.command);
        command.args(request.args);
        command.cwd(request.workspace);
        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| CaptureError::Pty(error.to_string()))?;
        drop(pair.slave);

        let output_limit = request.output_limit;
        let reader_thread = thread::spawn(move || read_bounded(&mut reader, output_limit));
        let mut cancelled = false;
        let mut timed_out = false;
        let deadline = Instant::now()
            .checked_add(request.timeout)
            .unwrap_or_else(Instant::now);
        let process_id = child.process_id();
        let mut stop_requested_at = None;
        let mut force_killed = false;
        let mut status: Option<ExitStatus> = None;
        loop {
            if status.is_none() {
                status = child.try_wait().map_err(CaptureError::Io)?;
            }

            let now = Instant::now();
            if stop_requested_at.is_none() && status.is_none() {
                if cancel.is_cancelled() {
                    cancelled = true;
                    interrupt_child(child.as_mut(), process_id).map_err(CaptureError::Io)?;
                    stop_requested_at = Some(now);
                } else if now >= deadline {
                    timed_out = true;
                    interrupt_child(child.as_mut(), process_id).map_err(CaptureError::Io)?;
                    stop_requested_at = Some(now);
                }
            }

            if let Some(requested_at) = stop_requested_at {
                if !force_killed && now.duration_since(requested_at) >= TERMINATION_GRACE {
                    force_kill_child(child.as_mut(), process_id).map_err(CaptureError::Io)?;
                    force_killed = true;
                }
                if force_killed && status.is_some() {
                    break;
                }
            } else if status.is_some() {
                break;
            }

            thread::sleep(POLL_INTERVAL);
        }
        let status = status.expect("capture loop exits only after child status is available");
        let (output, output_bytes, output_truncated) = reader_thread
            .join()
            .map_err(|_| CaptureError::Pty("PTY reader thread panicked".to_owned()))?
            .map_err(CaptureError::Io)?;

        Ok(PtyRunResult {
            exit_code: status.exit_code(),
            signal: status.signal().map(str::to_owned),
            output,
            output_bytes,
            output_truncated,
            cancelled,
            timed_out,
            terminal_size,
        })
    }
}

#[cfg(unix)]
fn interrupt_child(
    child: &mut (dyn Child + Send + Sync),
    process_id: Option<u32>,
) -> std::io::Result<()> {
    match process_id {
        Some(process_id) => signal_process_group(process_id, libc::SIGINT),
        None => child.kill(),
    }
}

#[cfg(not(unix))]
fn interrupt_child(
    child: &mut (dyn Child + Send + Sync),
    _process_id: Option<u32>,
) -> std::io::Result<()> {
    child.kill()
}

#[cfg(unix)]
fn force_kill_child(
    child: &mut (dyn Child + Send + Sync),
    process_id: Option<u32>,
) -> std::io::Result<()> {
    match process_id {
        Some(process_id) => signal_process_group(process_id, libc::SIGKILL),
        None => child.kill(),
    }
}

#[cfg(not(unix))]
fn force_kill_child(
    child: &mut (dyn Child + Send + Sync),
    _process_id: Option<u32>,
) -> std::io::Result<()> {
    child.kill()
}

#[cfg(unix)]
fn signal_process_group(process_id: u32, signal: libc::c_int) -> std::io::Result<()> {
    let process_id = i32::try_from(process_id).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "child process id does not fit in pid_t",
        )
    })?;
    // portable-pty calls setsid before exec on Unix, so the child PID is
    // also the process-group ID. A negative PID addresses the whole group.
    let result = unsafe { libc::kill(-process_id, signal) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error)
    }
}

fn read_bounded(reader: &mut dyn Read, limit: usize) -> std::io::Result<(Vec<u8>, usize, bool)> {
    let mut retained = Vec::with_capacity(limit.min(8 * 1024));
    let mut total = 0usize;
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                total = total.saturating_add(read);
                let keep = read.min(limit.saturating_sub(retained.len()));
                retained.extend_from_slice(&buffer[..keep]);
            }
            Err(error) if is_normal_pty_eof(&error) => break,
            Err(error) => return Err(error),
        }
    }
    let truncated = total > retained.len();
    Ok((retained, total, truncated))
}

#[cfg(unix)]
fn is_normal_pty_eof(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(libc::EIO)
}

#[cfg(not(unix))]
fn is_normal_pty_eof(_error: &std::io::Error) -> bool {
    false
}

impl From<PtySize> for TerminalSize {
    fn from(value: PtySize) -> Self {
        Self {
            rows: value.rows,
            cols: value.cols,
            pixel_width: value.pixel_width,
            pixel_height: value.pixel_height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ErrorReader {
        raw_os_error: Option<i32>,
    }

    impl Read for ErrorReader {
        fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
            match self.raw_os_error {
                Some(code) => Err(std::io::Error::from_raw_os_error(code)),
                None => Err(std::io::Error::other("fake PTY read failure")),
            }
        }
    }

    #[test]
    fn read_bounded_propagates_non_eof_errors() {
        let error = read_bounded(&mut ErrorReader { raw_os_error: None }, 1024)
            .expect_err("non-EOF read errors must be returned");

        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        assert_eq!(error.to_string(), "fake PTY read failure");
    }

    #[cfg(unix)]
    #[test]
    fn read_bounded_treats_unix_pty_eio_as_eof() {
        let result = read_bounded(
            &mut ErrorReader {
                raw_os_error: Some(libc::EIO),
            },
            1024,
        )
        .expect("EIO is the normal Unix PTY closure condition");

        assert_eq!(result, (Vec::new(), 0, false));
    }
}
