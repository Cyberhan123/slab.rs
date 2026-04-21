use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::audio::schema::{
    AudioTranscriptionRequest, AudioTranscriptionTaskResponse, TranscribeDecodeRequest,
    TranscribeVadRequest,
};
use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::api::validation::ValidatedJson;
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::AudioService;

#[derive(OpenApi)]
#[openapi(
    paths(transcribe, list_audio_transcriptions, get_audio_transcription),
    components(schemas(
        AudioTranscriptionRequest,
        AudioTranscriptionTaskResponse,
        TranscribeVadRequest,
        TranscribeDecodeRequest,
        OperationAcceptedResponse
    ))
)]
pub struct AudioApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/audio/transcriptions", post(transcribe).get(list_audio_transcriptions))
        .route("/audio/transcriptions/{id}", axum::routing::get(get_audio_transcription))
}

#[utoipa::path(
    post,
    path = "/v1/audio/transcriptions",
    tag = "audio",
    request_body(content = AudioTranscriptionRequest, description = "Audio transcription request"),
    responses(
        (status = 202, description = "Task accepted", body = OperationAcceptedResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn transcribe(
    State(service): State<AudioService>,
    ValidatedJson(req): ValidatedJson<AudioTranscriptionRequest>,
) -> Result<(StatusCode, Json<OperationAcceptedResponse>), ServerError> {
    let response = service.transcribe(req.into()).await?;
    Ok((StatusCode::ACCEPTED, Json(response.into())))
}

#[utoipa::path(
    get,
    path = "/v1/audio/transcriptions",
    tag = "audio",
    responses(
        (status = 200, description = "Audio transcription tasks listed", body = [AudioTranscriptionTaskResponse]),
        (status = 500, description = "Backend error"),
    )
)]
async fn list_audio_transcriptions(
    State(service): State<AudioService>,
) -> Result<Json<Vec<AudioTranscriptionTaskResponse>>, ServerError> {
    Ok(Json(service.list_transcription_tasks().await?.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    get,
    path = "/v1/audio/transcriptions/{id}",
    tag = "audio",
    params(("id" = String, Path, description = "Audio transcription task ID")),
    responses(
        (status = 200, description = "Audio transcription task detail", body = AudioTranscriptionTaskResponse),
        (status = 404, description = "Task not found"),
        (status = 500, description = "Backend error"),
    )
)]
async fn get_audio_transcription(
    State(service): State<AudioService>,
    Path(id): Path<String>,
) -> Result<Json<AudioTranscriptionTaskResponse>, ServerError> {
    Ok(Json(service.get_transcription_task(&id).await?.into()))
}
