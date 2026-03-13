use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::ffmpeg::schema::ConvertRequest;
use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::context::{AppState, WorkerState};
use crate::error::ServerError;
use crate::services::ffmpeg::FfmpegService;

#[derive(OpenApi)]
#[openapi(
    paths(convert),
    components(schemas(ConvertRequest, OperationAcceptedResponse))
)]
pub struct FfmpegApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/ffmpeg/convert", post(convert))
}

#[utoipa::path(
    post,
    path = "/v1/ffmpeg/convert",
    tag = "ffmpeg",
    request_body = ConvertRequest,
    responses(
        (status = 202, description = "Conversion task accepted", body = OperationAcceptedResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn convert(
    State(state): State<WorkerState>,
    Json(req): Json<ConvertRequest>,
) -> Result<(StatusCode, Json<OperationAcceptedResponse>), ServerError> {
    let service = FfmpegService::new(state);
    let response = service.convert(req).await?;
    Ok((StatusCode::ACCEPTED, Json(response)))
}
