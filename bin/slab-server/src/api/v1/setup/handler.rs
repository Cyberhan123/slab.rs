use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::setup::schema::{CompleteSetupRequest, SetupStatusResponse};
use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::SetupService;

#[derive(OpenApi)]
#[openapi(
    paths(setup_status, download_ffmpeg, complete_setup),
    components(schemas(
        SetupStatusResponse,
        crate::api::v1::setup::schema::ComponentStatusResponse,
        CompleteSetupRequest,
        OperationAcceptedResponse,
    ))
)]
pub struct SetupApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/setup/status", get(setup_status))
        .route("/setup/ffmpeg/download", post(download_ffmpeg))
        .route("/setup/complete", post(complete_setup))
}

/// Return the current environment status (FFmpeg, backends, initialized flag).
#[utoipa::path(
    get,
    path = "/v1/setup/status",
    tag = "setup",
    responses(
        (status = 200, description = "Current environment status", body = SetupStatusResponse),
        (status = 500, description = "Internal error"),
    )
)]
async fn setup_status(
    State(service): State<SetupService>,
) -> Result<Json<SetupStatusResponse>, ServerError> {
    Ok(Json(service.environment_status().await?.into()))
}

/// Kick off an FFmpeg download task.  Returns immediately with a task ID;
/// poll `GET /v1/tasks/{id}` to track progress.
#[utoipa::path(
    post,
    path = "/v1/setup/ffmpeg/download",
    tag = "setup",
    responses(
        (status = 202, description = "FFmpeg download task accepted", body = OperationAcceptedResponse),
        (status = 500, description = "Internal error"),
    )
)]
async fn download_ffmpeg(
    State(service): State<SetupService>,
) -> Result<(StatusCode, Json<OperationAcceptedResponse>), ServerError> {
    let op = service.download_ffmpeg().await?;
    Ok((StatusCode::ACCEPTED, Json(OperationAcceptedResponse { operation_id: op.operation_id })))
}

/// Mark the one-time setup as complete (or reset it).
#[utoipa::path(
    post,
    path = "/v1/setup/complete",
    tag = "setup",
    request_body = CompleteSetupRequest,
    responses(
        (status = 200, description = "Setup state saved; returns updated environment status", body = SetupStatusResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal error"),
    )
)]
async fn complete_setup(
    State(service): State<SetupService>,
    Json(body): Json<CompleteSetupRequest>,
) -> Result<Json<SetupStatusResponse>, ServerError> {
    Ok(Json(service.complete_setup(body.into()).await?.into()))
}
