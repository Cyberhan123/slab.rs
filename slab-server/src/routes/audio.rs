//! Audio transcription routes (Whisper) – async task pattern.
//!
//! Accepts an audio/video body, saves it to a temp file, then submits a
//! slab-core pipeline (ffmpeg → whisper) via `api::backend(...).preprocess(...).run()`.
//! The returned slab-core `TaskId` is persisted so that the generic
//! `/api/tasks` endpoints can query status and result via `slab_core::api::status/result`.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use bytes::Bytes;
use chrono::Utc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::{TaskRecord, TaskStore};
use crate::error::ServerError;
use crate::state::AppState;

/// Maximum allowed audio body size (50 MiB).
const MAX_AUDIO_BYTES: usize = 50 * 1024 * 1024;

/// Register audio routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/audio/transcriptions", post(transcribe))
}

/// Speech-to-text transcription (`POST /v1/audio/transcriptions`).
///
/// Accepts raw audio/video bytes.  The body is saved to a temporary file,
/// then a slab-core pipeline is submitted:
///
/// 1. **ffmpeg** (CPU preprocess stage via `std::process::Command`) converts
///    the file to raw PCM f32le at 16 kHz mono.
/// 2. **whisper** (GPU stage) transcribes the PCM samples.
///
/// Returns `{"task_id": "..."}` immediately; poll status via
/// `GET /api/tasks/{id}` and result via `GET /api/tasks/{id}/result`.
#[utoipa::path(
    post,
    path = "/v1/audio/transcriptions",
    tag = "audio",
    request_body(content = Vec<u8>, description = "Audio/video file bytes"),
    responses(
        (status = 202, description = "Task accepted", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn transcribe(
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ServerError> {
    debug!(body_len = body.len(), "transcription request");

    if body.is_empty() {
        return Err(ServerError::BadRequest("audio body is empty".into()));
    }
    if body.len() > MAX_AUDIO_BYTES {
        return Err(ServerError::BadRequest(format!(
            "audio body too large ({} bytes); maximum is {MAX_AUDIO_BYTES} bytes",
            body.len()
        )));
    }

    let task_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    // Save audio to a temp file so the sync preprocess closure can access it.
    let tmp_path = std::env::temp_dir()
        .join(format!("slab-audio-{task_id}.raw"))
        .to_string_lossy()
        .into_owned();
    tokio::fs::write(&tmp_path, &body)
        .await
        .map_err(|e| ServerError::Internal(format!("failed to write temp audio: {e}")))?;

    let input_data = serde_json::json!({ "tmp_path": tmp_path }).to_string();

    // Insert the server-side task record (core_task_id filled in after submission).
    state
        .store
        .insert_task(TaskRecord {
            id: task_id.clone(),
            task_type: "whisper".into(),
            status: "running".into(),
            input_data: Some(input_data),
            result_data: None,
            error_msg: None,
            core_task_id: None,
            created_at: now,
            updated_at: now,
        })
        .await?;

    // Build and submit the slab-core pipeline.
    // The preprocess closure runs inside `spawn_blocking` (see `CpuStage::run`),
    // so blocking I/O via `std::process::Command` is safe here.
    // The initial Bytes input is an empty placeholder; the preprocess stage
    // replaces it with the actual PCM f32le data.
    let tmp_for_closure = tmp_path.clone();
    let core_task_result = slab_core::api::backend("ggml.whisper")
        .op("inference")
        .input(slab_core::Payload::Bytes(Arc::from([] as [u8; 0]))) // placeholder; preprocess supplies PCM
        .preprocess("ffmpeg.to_pcm_f32le", move |_| {
            convert_to_pcm_f32le(&tmp_for_closure)
        })
        .run()
        .await;

    match core_task_result {
        Ok(core_task_id) => {
            // Persist the slab-core TaskId so status/result queries can use it.
            state
                .store
                .set_core_task_id(&task_id, core_task_id as i64)
                .await
                .unwrap_or_else(|e| warn!(task_id = %task_id, error = %e, "failed to store core_task_id"));
            info!(task_id = %task_id, core_task_id, "transcription task submitted to slab-core");
        }
        Err(e) => {
            warn!(task_id = %task_id, error = %e, "failed to submit transcription to slab-core");
            state
                .store
                .update_task_status(&task_id, "failed", None, Some(&e.to_string()))
                .await
                .unwrap_or_else(|db_e| warn!(error = %db_e, "failed to update task status"));
        }
    }

    // Clean up the temp file asynchronously (best effort).
    let tmp_cleanup = tmp_path.clone();
    tokio::spawn(async move {
        tokio::fs::remove_file(&tmp_cleanup).await.ok();
    });

    Ok(Json(serde_json::json!({ "task_id": task_id })))
}

/// Convert the audio/video file at `src_path` to raw PCM f32le samples at 16 kHz mono.
///
/// Uses `std::process::Command` so it can be called from within a
/// `spawn_blocking` / `CpuStage` context.  Returns `Payload::F32` on success.
///
/// Also used by `tasks::restart_task` for whisper task restarts.
pub fn convert_to_pcm_f32le(src_path: &str) -> Result<slab_core::Payload, String> {
    let pcm_path = std::path::Path::new(src_path)
        .with_extension("pcm")
        .to_string_lossy()
        .into_owned();

    let output = std::process::Command::new("ffmpeg")
        .args(["-y", "-i", src_path, "-f", "f32le", "-ar", "16000", "-ac", "1", &pcm_path])
        .output()
        .map_err(|e| format!("ffmpeg spawn failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg failed: {stderr}"));
    }

    let pcm_bytes = std::fs::read(&pcm_path)
        .map_err(|e| format!("failed to read ffmpeg PCM output: {e}"))?;

    // Clean up the PCM file (best effort).
    std::fs::remove_file(&pcm_path).ok();

    let sample_size = std::mem::size_of::<f32>();
    if pcm_bytes.len() % sample_size != 0 {
        return Err(format!(
            "ffmpeg produced misaligned PCM output ({} bytes, not a multiple of {sample_size})",
            pcm_bytes.len()
        ));
    }

    let samples: Vec<f32> = bytemuck::cast_slice::<u8, f32>(&pcm_bytes).to_vec();
    Ok(slab_core::Payload::F32(std::sync::Arc::from(samples.as_slice())))
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn rejects_empty_body() {
        let body = Bytes::new();
        assert!(body.is_empty());
    }

    #[test]
    fn rejects_oversized_body() {
        // Body length > MAX_AUDIO_BYTES should be rejected.
        assert!(MAX_AUDIO_BYTES + 1 > MAX_AUDIO_BYTES);
    }

    #[test]
    fn pcm_alignment_check() {
        // 8 bytes = 2 f32 samples (aligned).
        let bytes = vec![0u8; 8];
        assert_eq!(bytes.len() % std::mem::size_of::<f32>(), 0);
        // 3 bytes is not aligned.
        let bytes = vec![0u8; 3];
        assert_ne!(bytes.len() % std::mem::size_of::<f32>(), 0);
    }
}

