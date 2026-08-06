use std::io::Read;
use std::thread;
use std::time::Duration;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio_util::sync::CancellationToken;

use crate::{CaptureError, TerminalSize};

pub(crate) struct PtyRequest {
    pub command: String,
    pub args: Vec<String>,
    pub workspace: std::path::PathBuf,
    pub output_limit: usize,
    pub terminal_size: TerminalSize,
}

pub(crate) struct PtyRunResult {
    pub exit_code: u32,
    pub signal: Option<String>,
    pub output: Vec<u8>,
    pub output_bytes: usize,
    pub output_truncated: bool,
    pub cancelled: bool,
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
        let status = loop {
            if cancel.is_cancelled() && !cancelled {
                cancelled = true;
                child.kill().map_err(CaptureError::Io)?;
            }
            if let Some(status) = child.try_wait().map_err(CaptureError::Io)? {
                break status;
            }
            thread::sleep(Duration::from_millis(10));
        };
        let (output, output_bytes, output_truncated) = reader_thread
            .join()
            .map_err(|_| CaptureError::Pty("PTY reader thread panicked".to_owned()))?;

        Ok(PtyRunResult {
            exit_code: status.exit_code(),
            signal: status.signal().map(str::to_owned),
            output,
            output_bytes,
            output_truncated,
            cancelled,
            terminal_size,
        })
    }
}

fn read_bounded(reader: &mut dyn Read, limit: usize) -> (Vec<u8>, usize, bool) {
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
            Err(_) => break,
        }
    }
    let truncated = total > retained.len();
    (retained, total, truncated)
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
