//! Read-only local API for the SkillTape Console.

mod coverage;
mod deliveries;
mod read_model;
mod receipt_comparison;
pub mod routes;

pub use coverage::TapeCoverage;
pub use read_model::{ConsoleReadModel, ReadModelError};
pub use receipt_comparison::ReceiptComparison;
pub use routes::{router, router_with_static, ApiError};

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use thiserror::Error;
use tokio::net::TcpListener;

#[derive(Debug, Error)]
pub enum ServeError {
    #[error("Console API must bind to a loopback address")]
    NonLoopback,
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
        return Err(ServeError::NonLoopback);
    }
    let model = ConsoleReadModel::new(root)?;
    let static_root = static_root.map(validate_static_root).transpose()?;
    let listener = TcpListener::bind(bind).await.map_err(ServeError::Bind)?;
    let address = listener.local_addr().map_err(ServeError::Bind)?;
    println!("SkillTape Console API listening at http://{address}");
    use std::io::Write;
    std::io::stdout().flush().map_err(ServeError::Bind)?;
    let application =
        routes::router_with_static(model, static_root).layer(axum::middleware::from_fn(
            move |request: axum::extract::Request, next: axum::middleware::Next| async move {
                use axum::response::IntoResponse;
                if !local_browser_request(request.headers(), address.port()) {
                    return (
                        axum::http::StatusCode::FORBIDDEN,
                        axum::Json(serde_json::json!({"error":{"code":"LOCAL_ORIGIN_REQUIRED"}})),
                    )
                        .into_response();
                }
                next.run(request).await
            },
        ));
    axum::serve(listener, application)
        .await
        .map_err(ServeError::Server)
}

fn local_authority(value: &str, port: u16) -> Option<(String, u16)> {
    if value.contains(['@', '/', '\\']) {
        return None;
    }
    let authority = value.parse::<axum::http::uri::Authority>().ok()?;
    let host = authority
        .host()
        .trim_matches(['[', ']'])
        .to_ascii_lowercase();
    let actual_port = authority.port_u16().unwrap_or(80);
    let loopback = host == "localhost"
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    (loopback && actual_port == port).then_some((host, actual_port))
}

fn local_browser_request(headers: &axum::http::HeaderMap, port: u16) -> bool {
    let Some(host) = headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| local_authority(value, port))
    else {
        return false;
    };
    match headers.get(axum::http::header::ORIGIN) {
        None => true,
        Some(value) => value
            .to_str()
            .ok()
            .and_then(|value| value.parse::<axum::http::Uri>().ok())
            .is_some_and(|origin| {
                origin.scheme_str() == Some("http")
                    && origin.path() == "/"
                    && origin.query().is_none()
                    && origin
                        .authority()
                        .and_then(|value| local_authority(value.as_str(), port))
                        .is_some_and(|value| value == host)
            }),
    }
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
