use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{middleware, Json, Router};
use utoipa::OpenApi;

use crate::api::middleware::auth;
use crate::api::v1::backend::schema::{
    BackendListResponse, BackendStatusResponse, BackendTypeQuery, DownloadLibRequest,
    ReloadLibRequest,
};
use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::context::{AppState, ModelState, WorkerState};
use crate::error::ServerError;
use crate::services::backend::BackendService;

#[derive(OpenApi)]
#[openapi(
    paths(backend_status, list_backends, download_lib, reload_lib),
    components(schemas(
        DownloadLibRequest,
        ReloadLibRequest,
        BackendTypeQuery,
        BackendStatusResponse,
        BackendListResponse,
        OperationAcceptedResponse,
    ))
)]
pub struct BackendApi;

pub fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/backends", get(list_backends))
        .route("/backends/status", get(backend_status))
        .route("/backends/download", post(download_lib))
        .route("/backends/reload", post(reload_lib))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/v1/backends/status",
    tag = "backends",
    params(BackendTypeQuery),
    responses(
        (status = 200, description = "Backend worker is running", body = BackendStatusResponse),
        (status = 400, description = "Unknown model type"),
        (status = 401, description = "Unauthorised (admin token required)"),
    )
)]
async fn backend_status(
    State(model_state): State<ModelState>,
    State(worker_state): State<WorkerState>,
    Query(query): Query<BackendTypeQuery>,
) -> Result<Json<BackendStatusResponse>, ServerError> {
    let service = BackendService::new(model_state, worker_state);
    Ok(Json(service.backend_status(query).await?))
}

#[utoipa::path(
    get,
    path = "/v1/backends",
    tag = "backends",
    responses(
        (status = 200, description = "List of all registered backends", body = BackendListResponse),
        (status = 401, description = "Unauthorised (admin token required)"),
    )
)]
async fn list_backends(
    State(model_state): State<ModelState>,
    State(worker_state): State<WorkerState>,
) -> Result<Json<BackendListResponse>, ServerError> {
    let service = BackendService::new(model_state, worker_state);
    Ok(Json(service.list_backends().await?))
}

#[utoipa::path(
    post,
    path = "/v1/backends/download",
    tag = "backends",
    request_body = DownloadLibRequest,
    responses(
        (status = 202, description = "Download task accepted", body = OperationAcceptedResponse),
        (status = 400, description = "Bad request (invalid path)"),
        (status = 401, description = "Unauthorised (management token required)"),
    )
)]
async fn download_lib(
    State(model_state): State<ModelState>,
    State(worker_state): State<WorkerState>,
    Json(req): Json<DownloadLibRequest>,
) -> Result<(StatusCode, Json<OperationAcceptedResponse>), ServerError> {
    let service = BackendService::new(model_state, worker_state);
    let response = service.download_lib(req).await?;
    Ok((StatusCode::ACCEPTED, Json(response)))
}

#[utoipa::path(
    post,
    path = "/v1/backends/reload",
    tag = "backends",
    request_body = ReloadLibRequest,
    responses(
        (status = 200, description = "Backend reloaded with new library", body = BackendStatusResponse),
        (status = 400, description = "Bad request (invalid path or unknown backend)"),
        (status = 401, description = "Unauthorised (management token required)"),
    )
)]
async fn reload_lib(
    State(model_state): State<ModelState>,
    State(worker_state): State<WorkerState>,
    Json(req): Json<ReloadLibRequest>,
) -> Result<Json<BackendStatusResponse>, ServerError> {
    let service = BackendService::new(model_state, worker_state);
    Ok(Json(service.reload_lib(req).await?))
}
