use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::audio::schema::{
    CompletionRequest, TranscribeDecodeRequest, TranscribeVadRequest,
};
use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::api::validation::ValidatedJson;
use crate::context::AppState;
use crate::domain::services::AudioService;
use crate::error::ServerError;

#[derive(OpenApi)]
#[openapi(
    paths(transcribe),
    components(schemas(
        CompletionRequest,
        TranscribeVadRequest,
        TranscribeDecodeRequest,
        OperationAcceptedResponse
    ))
)]
pub struct AudioApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/audio/transcriptions", post(transcribe))
}

#[utoipa::path(
    post,
    path = "/v1/audio/transcriptions",
    tag = "audio",
    request_body(content = CompletionRequest, description = "Audio transcription request"),
    responses(
        (status = 202, description = "Task accepted", body = OperationAcceptedResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn transcribe(
    State(service): State<AudioService>,
    ValidatedJson(req): ValidatedJson<CompletionRequest>,
) -> Result<(StatusCode, Json<OperationAcceptedResponse>), ServerError> {
    let response = service.transcribe(req.into()).await?;
    Ok((StatusCode::ACCEPTED, Json(response.into())))
}
