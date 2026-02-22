//! Audio transcription routes (Whisper).
//!
//! Accepts raw PCM f32-le audio bytes and forwards them to the `ggml.whisper`
//! backend in slab-core.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use bytes::Bytes;
use tracing::{debug, info};

use crate::error::ServerError;
use crate::models::openai::TranscriptionResponse;
use crate::state::AppState;

/// Register audio routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/audio/transcriptions", post(transcribe))
}

/// Speech-to-text transcription (`POST /v1/audio/transcriptions`).
///
/// Accepts raw PCM f32-le bytes in the request body and returns the
/// transcribed text as JSON.
#[utoipa::path(
    post,
    path = "/v1/audio/transcriptions",
    tag = "audio",
    request_body(content = Vec<u8>, description = "Raw PCM f32-le audio bytes"),
    responses(
        (status = 200, description = "Transcription result", body = TranscriptionResponse),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn transcribe(
    State(_state): State<Arc<AppState>>,
    body: Bytes,
) -> Result<Json<TranscriptionResponse>, ServerError> {
    debug!(body_len = body.len(), "transcription request");

    // Reinterpret the raw bytes as f32 PCM samples.
    let samples: Vec<f32> = bytemuck::cast_slice::<u8, f32>(&body).to_vec();

    let result_bytes = slab_core::api::backend("ggml.whisper")
        .op("inference")
        .input(slab_core::Payload::F32(std::sync::Arc::from(
            samples.as_slice(),
        )))
        .run_wait()
        .await
        .map_err(ServerError::Runtime)?;

    let text = String::from_utf8(result_bytes.to_vec())
        .map_err(|e| ServerError::Internal(format!("backend returned invalid UTF-8: {e}")))?;

    info!(text_len = text.len(), "transcription done");

    Ok(Json(TranscriptionResponse { text }))
}
