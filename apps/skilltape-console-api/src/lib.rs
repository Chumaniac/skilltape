//! Read-only local API for the SkillTape Console.

mod read_model;
pub mod routes;

pub use read_model::{ConsoleReadModel, ReadModelError};
pub use routes::{router, router_with_static, ApiError};

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use thiserror::Error;
use tokio::net::TcpListener;

#[derive(Debug, Error)]
pub enum ServeError {
    #[error("console API model failed to initialize")]
    Model(#[from] ReadModelError),
    #[error("console API bind failed")]
    Bind(std::io::Error),
    #[error("console API server failed")]
    Server(std::io::Error),
    #[error("console static root is not a directory containing index.html")]
    StaticRoot,
}

pub async fn serve(root: impl AsRef<Path>, bind: SocketAddr) -> Result<(), ServeError> {
    serve_with_static(root, bind, None).await
}

pub async fn serve_with_static(
    root: impl AsRef<Path>,
    bind: SocketAddr,
    static_root: Option<&Path>,
) -> Result<(), ServeError> {
    if !bind.ip().is_loopback() {
        eprintln!(
            "warning: SkillTape Console API is bound outside loopback at {}; keep the workspace private",
            bind
        );
    }
    let model = ConsoleReadModel::new(root)?;
    let static_root = static_root.map(validate_static_root).transpose()?;
    let listener = TcpListener::bind(bind).await.map_err(ServeError::Bind)?;
    let address = listener.local_addr().map_err(ServeError::Bind)?;
    println!("SkillTape Console API listening at http://{address}");
    use std::io::Write;
    std::io::stdout().flush().map_err(ServeError::Bind)?;
    axum::serve(listener, routes::router_with_static(model, static_root))
        .await
        .map_err(ServeError::Server)
}

fn validate_static_root(path: &Path) -> Result<PathBuf, ServeError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| ServeError::StaticRoot)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ServeError::StaticRoot);
    }
    let index = path.join("index.html");
    let index_metadata = std::fs::symlink_metadata(&index).map_err(|_| ServeError::StaticRoot)?;
    if index_metadata.file_type().is_symlink() || !index_metadata.is_file() {
        return Err(ServeError::StaticRoot);
    }
    path.canonicalize().map_err(|_| ServeError::StaticRoot)
}
