//! Read-only local API for the SkillTape Console.

mod read_model;
pub mod routes;

pub use read_model::{ConsoleReadModel, ReadModelError};
pub use routes::{router, ApiError};

use std::net::SocketAddr;
use std::path::Path;

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
}

pub async fn serve(root: impl AsRef<Path>, bind: SocketAddr) -> Result<(), ServeError> {
    if !bind.ip().is_loopback() {
        eprintln!(
            "warning: SkillTape Console API is bound outside loopback at {}; keep the workspace private",
            bind
        );
    }
    let model = ConsoleReadModel::new(root)?;
    let listener = TcpListener::bind(bind).await.map_err(ServeError::Bind)?;
    let address = listener.local_addr().map_err(ServeError::Bind)?;
    println!("SkillTape Console API listening at http://{address}");
    axum::serve(listener, router(model))
        .await
        .map_err(ServeError::Server)
}
