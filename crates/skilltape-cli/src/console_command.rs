use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

const API_BINARY_ENV: &str = "SKILLTAPE_CONSOLE_API_BIN";
const UI_DIST_ENV: &str = "SKILLTAPE_CONSOLE_UI_DIST";
const OPEN_COMMAND_ENV: &str = "SKILLTAPE_CONSOLE_OPEN_COMMAND";
const READY_PREFIX: &str = "SkillTape Console API listening at ";
const READY_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub struct ConsoleConfig {
    pub workspace: PathBuf,
    pub port: u16,
    pub open: bool,
}

#[derive(Debug, Error)]
enum ConsoleError {
    #[error("workspace is not a directory: {0}")]
    InvalidWorkspace(PathBuf),
    #[error("workspace root must not be a symlink: {0}")]
    SymlinkWorkspace(PathBuf),
    #[error("console UI dist is unavailable: {0}")]
    InvalidUiDist(PathBuf),
    #[error("console UI dist is unavailable; run `npm install && npm run build` in apps/skilltape-console or set {UI_DIST_ENV}")]
    MissingUiDist,
    #[error(
        "console API binary was not found; build skilltape-console-api or set {API_BINARY_ENV}"
    )]
    MissingApiBinary,
    #[error("console API binary is not executable: {0}")]
    InvalidApiBinary(PathBuf),
    #[error("port is unavailable on localhost: {0}")]
    PortUnavailable(u16),
    #[error("failed to start console API: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("console API failed before becoming ready")]
    ApiExited,
    #[error("console API did not become ready within {READY_TIMEOUT:?}")]
    ApiNotReady,
    #[error("failed to install Ctrl-C handler: {0}")]
    Signal(#[source] ctrlc::Error),
    #[error("failed to open console URL: {0}")]
    Open(#[source] std::io::Error),
}

pub fn run(config: ConsoleConfig) -> ExitCode {
    match run_console(config) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn run_console(config: ConsoleConfig) -> Result<ExitCode, ConsoleError> {
    let workspace = validate_workspace(&config.workspace)?;
    let static_root = resolve_ui_dist()?;

    if config.port != 0 {
        ensure_port_available(config.port)?;
    }

    let api_binary = resolve_api_binary()?;
    let mut child = spawn_api(&api_binary, &workspace, &static_root, config.port)?;
    let ready = match wait_until_ready(&mut child) {
        Ok(url) => url,
        Err(error) => {
            terminate_child(&mut child);
            return Err(error);
        }
    };

    println!("SkillTape Console listening at {ready}");
    println!("Read-only local viewer; it never executes commands or modifies workspace artifacts.");
    println!("Bound to loopback only; do not expose this port to an untrusted network.");
    println!("Press Ctrl-C to stop.");

    if config.open {
        if let Err(error) = open_url(&ready) {
            terminate_child(&mut child);
            return Err(error);
        }
    }

    let stop = Arc::new(AtomicBool::new(false));
    let signal_stop = Arc::clone(&stop);
    ctrlc::set_handler(move || signal_stop.store(true, Ordering::SeqCst))
        .map_err(ConsoleError::Signal)?;

    loop {
        if stop.load(Ordering::SeqCst) {
            terminate_child(&mut child);
            return Ok(ExitCode::from(130));
        }

        match child.try_wait().map_err(ConsoleError::Spawn)? {
            Some(status) => return Ok(exit_code(status.code())),
            None => thread::sleep(Duration::from_millis(50)),
        }
    }
}

fn validate_workspace(path: &Path) -> Result<PathBuf, ConsoleError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| ConsoleError::InvalidWorkspace(path.to_owned()))?;
    if metadata.file_type().is_symlink() {
        return Err(ConsoleError::SymlinkWorkspace(path.to_owned()));
    }
    if !metadata.is_dir() {
        return Err(ConsoleError::InvalidWorkspace(path.to_owned()));
    }
    fs::canonicalize(path).map_err(|_| ConsoleError::InvalidWorkspace(path.to_owned()))
}

fn resolve_ui_dist() -> Result<PathBuf, ConsoleError> {
    let override_path = env::var_os(UI_DIST_ENV).map(PathBuf::from);
    let current_dir = env::current_dir().ok();
    let current_exe = env::current_exe().ok();
    resolve_ui_dist_from(
        override_path.as_deref(),
        current_dir.as_deref(),
        current_exe.as_deref(),
    )
}

fn resolve_ui_dist_from(
    override_path: Option<&Path>,
    current_dir: Option<&Path>,
    current_exe: Option<&Path>,
) -> Result<PathBuf, ConsoleError> {
    if let Some(path) = override_path {
        return validate_ui_dist(path.to_owned());
    }

    let mut candidates = Vec::new();
    if let Some(current_dir) = current_dir {
        candidates.push(current_dir.join("apps/skilltape-console/dist"));
        candidates.push(current_dir.join("skilltape-console/dist"));
    }
    if let Some(current_exe) = current_exe {
        for ancestor in current_exe.ancestors() {
            candidates.push(ancestor.join("apps/skilltape-console/dist"));
            candidates.push(ancestor.join("console"));
        }
    }

    candidates
        .into_iter()
        .find_map(|candidate| validate_ui_dist(candidate).ok())
        .ok_or(ConsoleError::MissingUiDist)
}

fn validate_ui_dist(path: PathBuf) -> Result<PathBuf, ConsoleError> {
    let metadata =
        fs::symlink_metadata(&path).map_err(|_| ConsoleError::InvalidUiDist(path.clone()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ConsoleError::InvalidUiDist(path));
    }
    let index = path.join("index.html");
    let index_metadata =
        fs::symlink_metadata(&index).map_err(|_| ConsoleError::InvalidUiDist(path.clone()))?;
    if index_metadata.file_type().is_symlink() || !index_metadata.is_file() {
        return Err(ConsoleError::InvalidUiDist(path));
    }
    fs::canonicalize(path).map_err(|_| ConsoleError::InvalidUiDist(index))
}

fn resolve_api_binary() -> Result<PathBuf, ConsoleError> {
    if let Some(value) = env::var_os(API_BINARY_ENV) {
        let path = PathBuf::from(value);
        return validate_api_binary(path);
    }

    let binary_name = api_binary_name();
    let mut candidates = Vec::new();
    if let Ok(current_exe) = env::current_exe() {
        for ancestor in current_exe.ancestors() {
            candidates.push(ancestor.join(binary_name));
        }
    }
    if let Some(path_value) = env::var_os("PATH") {
        candidates.extend(env::split_paths(&path_value).map(|path| path.join(binary_name)));
    }

    candidates
        .into_iter()
        .find_map(|candidate| validate_api_binary(candidate).ok())
        .ok_or(ConsoleError::MissingApiBinary)
}

fn validate_api_binary(path: PathBuf) -> Result<PathBuf, ConsoleError> {
    if !path.is_file() {
        return Err(ConsoleError::InvalidApiBinary(path));
    }
    Ok(path)
}

fn api_binary_name() -> &'static str {
    if cfg!(windows) {
        "skilltape-console-api.exe"
    } else {
        "skilltape-console-api"
    }
}

fn ensure_port_available(port: u16) -> Result<(), ConsoleError> {
    TcpListener::bind(("127.0.0.1", port))
        .map(drop)
        .map_err(|_| ConsoleError::PortUnavailable(port))
}

fn spawn_api(
    binary: &Path,
    workspace: &Path,
    static_root: &Path,
    port: u16,
) -> Result<Child, ConsoleError> {
    Command::new(binary)
        .args(["--workspace", &workspace.to_string_lossy()])
        .args(["--bind", "127.0.0.1"])
        .args(["--port", &port.to_string()])
        .args(["--static-root", &static_root.to_string_lossy()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(ConsoleError::Spawn)
}

fn wait_until_ready(child: &mut Child) -> Result<String, ConsoleError> {
    let stdout = child.stdout.take().ok_or(ConsoleError::ApiExited)?;
    let lines = receive_lines(stdout);
    let deadline = Instant::now() + READY_TIMEOUT;

    loop {
        if let Ok(line) = lines.try_recv() {
            if let Some(url) = parse_ready_url(&line) {
                return Ok(url);
            }
            eprintln!("console API: {line}");
        }

        if child.try_wait().map_err(ConsoleError::Spawn)?.is_some() {
            return Err(ConsoleError::ApiExited);
        }
        if Instant::now() >= deadline {
            return Err(ConsoleError::ApiNotReady);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn receive_lines(stdout: impl std::io::Read + Send + 'static) -> Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) => {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    receiver
}

fn parse_ready_url(line: &str) -> Option<String> {
    let url = line.strip_prefix(READY_PREFIX)?.trim();
    let address = url.strip_prefix("http://")?;
    let address = address.parse::<std::net::SocketAddr>().ok()?;
    if !address.ip().is_loopback() {
        return None;
    }
    Some(format!("http://{address}"))
}

fn open_url(url: &str) -> Result<(), ConsoleError> {
    if let Some(program) = env::var_os(OPEN_COMMAND_ENV) {
        Command::new(program)
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(ConsoleError::Open)
    } else {
        #[cfg(target_os = "macos")]
        {
            Command::new("open")
                .arg(url)
                .spawn()
                .map(|_| ())
                .map_err(ConsoleError::Open)
        }
        #[cfg(target_os = "linux")]
        {
            Command::new("xdg-open")
                .arg(url)
                .spawn()
                .map(|_| ())
                .map_err(ConsoleError::Open)
        }
        #[cfg(target_os = "windows")]
        {
            Command::new("cmd")
                .args(["/C", "start", "", url])
                .spawn()
                .map(|_| ())
                .map_err(ConsoleError::Open)
        }
    }
}

fn terminate_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
}

fn exit_code(code: Option<i32>) -> ExitCode {
    match code.and_then(|value| u8::try_from(value).ok()) {
        Some(value) => ExitCode::from(value),
        None => ExitCode::FAILURE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn installed_ui_dist_is_found_next_to_the_cli_directory() {
        let temp = TempDir::new().expect("temporary directory");
        let bin = temp.path().join("bin");
        let console = temp.path().join("console");
        fs::create_dir_all(&bin).expect("bin directory");
        fs::create_dir_all(&console).expect("console directory");
        let executable = bin.join(api_binary_name());
        fs::write(&executable, b"api").expect("API fixture");
        fs::write(console.join("index.html"), b"<main>Console</main>").expect("index");

        let resolved = resolve_ui_dist_from(None, None, Some(&executable)).expect("UI dist");

        assert_eq!(resolved, console.canonicalize().expect("canonical console"));
    }
}
