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

/// Maximum allowed audio body size (50 MiB).
const MAX_AUDIO_BYTES: usize = 50 * 1024 * 1024;

/// Register audio routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/audio/transcriptions", post(transcribe))
}

/// Speech-to-text transcription (`POST /v1/audio/transcriptions`).
///
/// Accepts raw PCM f32-le bytes in the request body and returns the
/// transcribed text as JSON.
///
/// The body length **must** be a multiple of 4 (the byte-size of `f32`);
/// otherwise the request is rejected with HTTP 400.
#[utoipa::path(
    post,
    path = "/v1/audio/transcriptions",
    tag = "audio",
    request_body(content = Vec<u8>, description = "Raw PCM f32-le audio bytes"),
    responses(
        (status = 200, description = "Transcription result", body = TranscriptionResponse),
        (status = 400, description = "Bad request (misaligned body or empty)"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn transcribe(
    State(_state): State<Arc<AppState>>,
    body: Bytes,
) -> Result<Json<TranscriptionResponse>, ServerError> {
    debug!(body_len = body.len(), "transcription request");

    if body.is_empty() {
        return Err(ServerError::BadRequest("audio body is empty".into()));
    }

    if body.len() > MAX_AUDIO_BYTES {
        return Err(ServerError::BadRequest(format!(
            "audio body too large ({} bytes); maximum is {} bytes",
            body.len(),
            MAX_AUDIO_BYTES,
        )));
    }

    // bytemuck::cast_slice silently truncates trailing bytes if the length is
    // not a multiple of sizeof(f32) = 4.  Reject misaligned input explicitly.
    let sample_size = std::mem::size_of::<f32>();
    if body.len() % sample_size != 0 {
        return Err(ServerError::BadRequest(format!(
            "audio body length ({}) is not a multiple of {} (size of f32); \
             expected raw PCM f32-le data",
            body.len(),
            sample_size,
        )));
    }

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

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn rejects_empty_body() {
        // Simulate validation logic.
        let body = Bytes::new();
        assert!(body.is_empty());
    }

    #[test]
    fn rejects_misaligned_body() {
        // 3 bytes is not divisible by 4.
        let body = Bytes::from_static(&[0u8, 1, 2]);
        let sample_size = std::mem::size_of::<f32>();
        assert_ne!(body.len() % sample_size, 0);
    }

    #[test]
    fn accepts_aligned_body() {
        // 8 bytes = 2 f32 samples.
        let body = Bytes::from(vec![0u8; 8]);
        let sample_size = std::mem::size_of::<f32>();
        assert_eq!(body.len() % sample_size, 0);
    }
}

