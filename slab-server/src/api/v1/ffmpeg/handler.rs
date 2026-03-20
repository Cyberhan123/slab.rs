use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::ffmpeg::schema::ConvertRequest;
use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::api::validation::ValidatedJson;
use crate::context::AppState;
use crate::domain::services::FfmpegService;
use crate::error::ServerError;

#[derive(OpenApi)]
#[openapi(paths(convert), components(schemas(ConvertRequest, OperationAcceptedResponse)))]
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
    State(service): State<FfmpegService>,
    ValidatedJson(req): ValidatedJson<ConvertRequest>,
) -> Result<(StatusCode, Json<OperationAcceptedResponse>), ServerError> {
    if !tokio::fs::try_exists(&req.source_path).await.unwrap_or(false) {
        return Err(ServerError::BadRequest(format!(
            "source_path '{}' does not exist or is not accessible",
            req.source_path
        )));
    }

    let response = service.convert(req.into()).await?;
    Ok((StatusCode::ACCEPTED, Json(response.into())))
}
