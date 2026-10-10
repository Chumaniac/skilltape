use std::convert::Infallible;
use std::path::{Component, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{
    sse::{Event, Sse},
    IntoResponse, Response,
};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_stream::iter;

use crate::read_model::{
    normalize_page, Collection, ConsoleReadModel, ReadModelError, SkillDiff, StoredDocument,
    TapeEvents, CONSOLE_SCHEMA_V1,
};

#[derive(Clone, Debug)]
pub enum ApiError {
    BadRequest {
        code: &'static str,
        message: &'static str,
    },
    NotFound,
    Forbidden,
    InvalidDocument,
    Internal,
    Unavailable {
        code: &'static str,
        message: &'static str,
    },
}

#[derive(Clone, Debug, Deserialize, Default)]
struct PageQuery {
    offset: Option<String>,
    limit: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct ErrorResponse {
    schema: &'static str,
    error: ErrorDetail,
}

#[derive(Clone, Debug, Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: &'static str,
}

impl From<ReadModelError> for ApiError {
    fn from(error: ReadModelError) -> Self {
        match error {
            ReadModelError::UnsafeId => Self::BadRequest {
                code: "unsafe_id",
                message: "resource identifier is unsafe",
            },
            ReadModelError::UnsafePath => Self::Forbidden,
            ReadModelError::NotFound => Self::NotFound,
            ReadModelError::InvalidDocument => Self::InvalidDocument,
            ReadModelError::UnsupportedPlatform => Self::Unavailable {
                code: "unsupported_delivery_platform",
                message: "File integrity inspection is unavailable on this platform.",
            },
            ReadModelError::InvalidRoot | ReadModelError::Io(_) => Self::Internal,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::BadRequest { code, message } => (StatusCode::BAD_REQUEST, code, message),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "not_found",
                "requested resource was not found",
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                "unsafe_path",
                "requested resource path is not allowed",
            ),
            Self::InvalidDocument => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_document",
                "stored resource is invalid",
            ),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "console API could not read the workspace",
            ),
            Self::Unavailable { code, message } => (StatusCode::SERVICE_UNAVAILABLE, code, message),
        };
        (
            status,
            [(header::CONTENT_TYPE, "application/json")],
            Json(ErrorResponse {
                schema: "skilltape.dev/api-error/v1",
                error: ErrorDetail { code, message },
            }),
        )
            .into_response()
    }
}

pub fn router(model: ConsoleReadModel) -> Router {
    router_with_static(model, None)
}

#[derive(Clone, Debug)]
struct AppState {
    model: ConsoleReadModel,
    static_root: Option<PathBuf>,
    inspections: Arc<tokio::sync::Semaphore>,
}

pub fn router_with_static(model: ConsoleReadModel, static_root: Option<PathBuf>) -> Router {
    Router::new()
        .route("/api/v1/workspaces", get(list_workspaces))
        .route("/api/v1/workspaces/{id}/tapes", get(list_tapes))
        .route("/api/v1/workspaces/{id}/deliveries", get(list_deliveries))
        .route("/api/v1/deliveries/{id}", get(delivery_review))
        .route("/api/v1/tapes/{id}/events", get(tape_events))
        .route("/api/v1/skills/{id}/diff", get(skill_diff))
        .route("/api/v1/runs/{id}", get(run_document))
        .route("/api/v1/receipts/{id}", get(receipt_document))
        .route("/api/v1/runs/{id}/events", get(run_events))
        .route("/", get(index))
        .route("/{*path}", get(static_asset))
        .with_state(AppState {
            model,
            static_root,
            inspections: Arc::new(tokio::sync::Semaphore::new(2)),
        })
}

fn inspection_permit(state: &AppState) -> Result<tokio::sync::OwnedSemaphorePermit, ApiError> {
    Arc::clone(&state.inspections)
        .try_acquire_owned()
        .map_err(|_| ApiError::Unavailable {
            code: "inspection_busy",
            message: "Two inspections are active. Retry after they finish.",
        })
}

async fn inspect<T, F>(
    permit: tokio::sync::OwnedSemaphorePermit,
    work: F,
) -> Result<(T, tokio::sync::OwnedSemaphorePermit), ApiError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, ReadModelError> + Send + 'static,
{
    // The job and its result own the slot until the actual work has finished,
    // even if its HTTP caller cancels before either blocking stage returns.
    let (result, permit) = tokio::task::spawn_blocking(move || (work(), permit))
        .await
        .map_err(|_| ApiError::Internal)?;
    Ok((result?, permit))
}

async fn list_deliveries(
    State(state): State<AppState>,
    Path(workspace): Path<String>,
    Query(query): Query<PageQuery>,
) -> Result<Json<Collection<crate::deliveries::DeliverySummary>>, ApiError> {
    let (offset, limit) = page(&query)?;
    if workspace != crate::read_model::WORKSPACE_ID {
        return Err(ReadModelError::NotFound.into());
    }
    let permit = inspection_permit(&state)?;
    let (catalog, permit) = inspect(permit, move || state.model.delivery_catalog()).await?;
    let total = catalog.total();
    let selected = catalog.page(offset, limit)?;
    let (items, _permit) = inspect(permit, move || selected.read()).await?;
    let next = offset.saturating_add(items.len());
    Ok(Json(Collection {
        schema: CONSOLE_SCHEMA_V1,
        items,
        offset,
        limit,
        total,
        next_offset: (next < total).then_some(next),
    }))
}

async fn delivery_review(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<crate::deliveries::DeliveryReview>, ApiError> {
    crate::deliveries::validate_delivery_id(&id)?;
    let permit = inspection_permit(&state)?;
    let (catalog, permit) = inspect(permit, move || state.model.delivery_catalog()).await?;
    let selected = catalog.select(&id)?;
    let (report, _permit) = inspect(permit, move || selected.read()).await?;
    Ok(Json(report))
}

async fn list_workspaces(
    State(state): State<AppState>,
) -> Result<Json<Collection<crate::read_model::WorkspaceSummary>>, ApiError> {
    Ok(Json(state.model.workspaces()?))
}

async fn list_tapes(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(query): Query<PageQuery>,
) -> Result<Json<Collection<crate::read_model::TapeSummary>>, ApiError> {
    let (offset, limit) = page(&query)?;
    Ok(Json(state.model.tapes(&workspace_id, offset, limit)?))
}

async fn tape_events(
    State(state): State<AppState>,
    Path(tape_id): Path<String>,
    Query(query): Query<PageQuery>,
) -> Result<Json<TapeEvents>, ApiError> {
    let (offset, limit) = page(&query)?;
    Ok(Json(state.model.tape_events(&tape_id, offset, limit)?))
}

async fn skill_diff(
    State(state): State<AppState>,
    Path(skill_id): Path<String>,
) -> Result<Json<SkillDiff>, ApiError> {
    Ok(Json(state.model.skill_diff(&skill_id)?))
}

async fn run_document(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Result<Json<StoredDocument>, ApiError> {
    Ok(Json(state.model.run(&run_id)?))
}

async fn receipt_document(
    State(state): State<AppState>,
    Path(receipt_id): Path<String>,
) -> Result<Json<StoredDocument>, ApiError> {
    Ok(Json(state.model.receipt(&receipt_id)?))
}

async fn run_events(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    headers: HeaderMap,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let last_event_id = parse_last_event_id(&headers)?;
    let events = state.model.run_events(&run_id, last_event_id)?;
    let last_sequence = state.model.last_run_sequence(&run_id)?;
    let mut output = Vec::with_capacity(events.len() + 1);
    for event in events {
        let data = serde_json::to_string(&event.document).map_err(|_| ApiError::Internal)?;
        output.push(Ok(Event::default()
            .id(event.sequence.to_string())
            .event("run")
            .data(data)));
    }

    let terminal_sequence = last_sequence.map_or(0, |sequence| sequence.saturating_add(1));
    if last_event_id.is_none_or(|last| last < terminal_sequence) {
        let terminal = json!({
            "schema": "skilltape.dev/run-events/v1",
            "status": "complete",
            "last_sequence": last_sequence,
        });
        output.push(Ok(Event::default()
            .id(terminal_sequence.to_string())
            .event("end")
            .data(terminal.to_string())));
    }
    Ok(Sse::new(iter(output)))
}

async fn index(State(state): State<AppState>) -> Response {
    static_asset_bytes(state.static_root.as_deref(), "index.html").await
}

async fn static_asset(State(state): State<AppState>, Path(path): Path<String>) -> Response {
    static_asset_bytes(state.static_root.as_deref(), &path).await
}

async fn static_asset_bytes(root: Option<&std::path::Path>, relative: &str) -> Response {
    let Some(root) = root else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(path) = safe_static_path(root, relative) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            [(header::CONTENT_TYPE, static_content_type(&path))],
            Body::from(bytes),
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

fn safe_static_path(root: &std::path::Path, relative: &str) -> Option<PathBuf> {
    let mut path = root.to_owned();
    for component in std::path::Path::new(relative).components() {
        match component {
            Component::Normal(value) => path.push(value),
            Component::CurDir => {}
            Component::RootDir | Component::ParentDir | Component::Prefix(_) => return None,
        }
    }
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return None;
    }
    let canonical = path.canonicalize().ok()?;
    if !canonical.starts_with(root) {
        return None;
    }
    Some(path)
}

fn static_content_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("wasm") => "application/wasm",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn page(query: &PageQuery) -> Result<(usize, usize), ApiError> {
    let offset = parse_query_number(query.offset.as_deref(), "offset")?;
    let limit = parse_query_number(query.limit.as_deref(), "limit")?;
    normalize_page(offset, limit).map_err(|_| ApiError::BadRequest {
        code: "invalid_pagination",
        message: "offset must be non-negative and limit must be between 1 and 100",
    })
}

fn parse_query_number(value: Option<&str>, name: &'static str) -> Result<Option<usize>, ApiError> {
    match value {
        Some(value) => value
            .parse::<usize>()
            .map(Some)
            .map_err(|_| ApiError::BadRequest {
                code: "invalid_pagination",
                message: match name {
                    "offset" => "offset must be a non-negative integer",
                    _ => "limit must be a positive integer",
                },
            }),
        None => Ok(None),
    }
}

fn parse_last_event_id(headers: &HeaderMap) -> Result<Option<u64>, ApiError> {
    let Some(value) = headers.get("last-event-id") else {
        return Ok(None);
    };
    let value = value.to_str().map_err(|_| ApiError::BadRequest {
        code: "invalid_last_event_id",
        message: "Last-Event-ID must be an unsigned integer",
    })?;
    value
        .parse::<u64>()
        .map(Some)
        .map_err(|_| ApiError::BadRequest {
            code: "invalid_last_event_id",
            message: "Last-Event-ID must be an unsigned integer",
        })
}

#[cfg(test)]
mod inspection_tests {
    use super::*;

    #[tokio::test]
    async fn busy_inspections_do_not_build_a_queue() {
        let root = tempfile::TempDir::new().unwrap();
        let state = AppState {
            model: ConsoleReadModel::new(root.path()).unwrap(),
            static_root: None,
            inspections: Arc::new(tokio::sync::Semaphore::new(2)),
        };
        let first = inspection_permit(&state).unwrap();
        let second = inspection_permit(&state).unwrap();
        assert!(matches!(
            inspection_permit(&state),
            Err(ApiError::Unavailable {
                code: "inspection_busy",
                ..
            })
        ));
        drop(first);
        assert!(inspection_permit(&state).is_ok());
        drop(second);
    }

    #[tokio::test]
    async fn cancellation_keeps_the_slot_until_blocking_work_finishes() {
        let root = tempfile::TempDir::new().unwrap();
        let state = AppState {
            model: ConsoleReadModel::new(root.path()).unwrap(),
            static_root: None,
            inspections: Arc::new(tokio::sync::Semaphore::new(2)),
        };
        let first = inspection_permit(&state).unwrap();
        let permit = inspection_permit(&state).unwrap();
        let (started, begun) = tokio::sync::oneshot::channel();
        let (finish, finished) = std::sync::mpsc::channel();
        let task = tokio::spawn(async move {
            inspect(permit, move || {
                started.send(()).unwrap();
                finished
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .unwrap();
                Ok::<_, ReadModelError>(())
            })
            .await
        });
        begun.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_eq!(state.inspections.available_permits(), 0);
        finish.send(()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while state.inspections.available_permits() != 1 {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        drop(first);
        assert_eq!(state.inspections.available_permits(), 2);
    }
}
