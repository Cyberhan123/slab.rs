use std::sync::Arc;

use axum::extract::State;
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
use crate::api::validation::{ValidatedJson, ValidatedQuery};
use crate::context::AppState;
use crate::domain::models::{
    BackendStatusQuery, BackendStatusView, DownloadBackendLibCommand, ReloadBackendLibCommand,
};
use crate::domain::services::to_operation_accepted_response;
use crate::domain::services::BackendService;
use crate::error::ServerError;

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
    State(service): State<BackendService>,
    ValidatedQuery(query): ValidatedQuery<BackendTypeQuery>,
) -> Result<Json<BackendStatusResponse>, ServerError> {
    Ok(Json(to_backend_status_response(
        service
            .backend_status(BackendStatusQuery {
                backend_id: query.backend_id,
            })
            .await?,
    )))
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
    State(service): State<BackendService>,
) -> Result<Json<BackendListResponse>, ServerError> {
    let backends = service
        .list_backends()
        .await?
        .into_iter()
        .map(to_backend_status_response)
        .collect();
    Ok(Json(BackendListResponse { backends }))
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
    State(service): State<BackendService>,
    ValidatedJson(req): ValidatedJson<DownloadLibRequest>,
) -> Result<(StatusCode, Json<OperationAcceptedResponse>), ServerError> {
    let response = service
        .download_lib(DownloadBackendLibCommand {
            backend_id: req.backend_id,
            target_dir: req.target_dir,
        })
        .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(to_operation_accepted_response(response)),
    ))
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
    State(service): State<BackendService>,
    ValidatedJson(req): ValidatedJson<ReloadLibRequest>,
) -> Result<Json<BackendStatusResponse>, ServerError> {
    Ok(Json(to_backend_status_response(
        service
            .reload_lib(ReloadBackendLibCommand {
                backend_id: req.backend_id,
                lib_path: req.lib_path,
                model_path: req.model_path,
                num_workers: req.num_workers,
            })
            .await?,
    )))
}

fn to_backend_status_response(view: BackendStatusView) -> BackendStatusResponse {
    BackendStatusResponse {
        backend: view.backend,
        status: view.status,
    }
}
