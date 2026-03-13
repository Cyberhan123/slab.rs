use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::validation::ValidatedJson;
use crate::api::v1::audio::schema::{
    CompletionRequest, TranscribeDecodeRequest, TranscribeVadRequest,
};
use crate::api::v1::tasks::schema::OperationAcceptedResponse;
use crate::context::AppState;
use crate::domain::services::to_operation_accepted_response;
use crate::error::ServerError;
use crate::services::audio::{
    AudioService, AudioTranscriptionCommand, TranscribeDecodeOptions, TranscribeVadOptions,
};

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
    let response = service.transcribe(to_audio_command(req)).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(to_operation_accepted_response(response)),
    ))
}

fn to_audio_command(request: CompletionRequest) -> AudioTranscriptionCommand {
    AudioTranscriptionCommand {
        path: request.path,
        vad: request.vad.map(to_vad_options),
        decode: request.decode.map(to_decode_options),
    }
}

fn to_vad_options(request: TranscribeVadRequest) -> TranscribeVadOptions {
    TranscribeVadOptions {
        enabled: request.enabled,
        model_path: request.model_path,
        threshold: request.threshold,
        min_speech_duration_ms: request.min_speech_duration_ms,
        min_silence_duration_ms: request.min_silence_duration_ms,
        max_speech_duration_s: request.max_speech_duration_s,
        speech_pad_ms: request.speech_pad_ms,
        samples_overlap: request.samples_overlap,
    }
}

fn to_decode_options(request: TranscribeDecodeRequest) -> TranscribeDecodeOptions {
    TranscribeDecodeOptions {
        offset_ms: request.offset_ms,
        duration_ms: request.duration_ms,
        no_context: request.no_context,
        no_timestamps: request.no_timestamps,
        token_timestamps: request.token_timestamps,
        split_on_word: request.split_on_word,
        suppress_nst: request.suppress_nst,
        word_thold: request.word_thold,
        max_len: request.max_len,
        max_tokens: request.max_tokens,
        temperature: request.temperature,
        temperature_inc: request.temperature_inc,
        entropy_thold: request.entropy_thold,
        logprob_thold: request.logprob_thold,
        no_speech_thold: request.no_speech_thold,
        tdrz_enable: request.tdrz_enable,
    }
}
