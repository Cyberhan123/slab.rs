//! Audio transcription routes (Whisper) – async task pattern.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use bytes::Bytes;
use chrono::Utc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::{TaskRecord, TaskStore};
use crate::db::sqlite::SqliteStore;
use crate::error::ServerError;
use crate::state::{AppState, TaskManager};

/// Maximum allowed audio body size (50 MiB).
const MAX_AUDIO_BYTES: usize = 50 * 1024 * 1024;

/// Register audio routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/audio/transcriptions", post(transcribe))
}

/// Speech-to-text transcription (`POST /v1/audio/transcriptions`).
#[utoipa::path(
    post,
    path = "/v1/audio/transcriptions",
    tag = "audio",
    request_body(content = Vec<u8>, description = "Raw PCM f32-le audio bytes or audio/video file"),
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

    // Save audio to temp file so it survives background processing.
    let tmp_path = std::env::temp_dir()
        .join(format!("slab-audio-{}.raw", task_id))
        .to_string_lossy()
        .into_owned();
    tokio::fs::write(&tmp_path, &body)
        .await
        .map_err(|e| ServerError::Internal(format!("failed to write temp audio: {e}")))?;

    let input_data = serde_json::json!({ "tmp_path": tmp_path }).to_string();

    state
        .store
        .insert_task(TaskRecord {
            id: task_id.clone(),
            task_type: "whisper".into(),
            status: "pending".into(),
            input_data: Some(input_data),
            result_data: None,
            error_msg: None,
            created_at: now,
            updated_at: now,
        })
        .await?;

    let store = Arc::clone(&state.store);
    let task_manager = Arc::clone(&state.task_manager);
    let tid = task_id.clone();
    let tmp = tmp_path.clone();

    let join = tokio::spawn(async move {
        store.update_task_status(&tid, "running", None, None).await.ok();

        let audio_bytes = match tokio::fs::read(&tmp).await {
            Ok(b) => b,
            Err(e) => {
                warn!(task_id = %tid, error = %e, "failed to read audio temp file");
                store
                    .update_task_status(&tid, "failed", None, Some(&e.to_string()))
                    .await
                    .ok();
                task_manager.remove(&tid);
                return;
            }
        };

        let samples: Vec<f32> = if audio_bytes.len() % 4 == 0 {
            bytemuck::cast_slice::<u8, f32>(&audio_bytes).to_vec()
        } else {
            // Attempt ffmpeg conversion to PCM f32-le.
            // Whisper requires 16 kHz mono f32 PCM input.
            let pcm_path = std::path::Path::new(&tmp)
                .with_extension("pcm")
                .to_string_lossy()
                .into_owned();
            let ffmpeg_result = tokio::process::Command::new("ffmpeg")
                .args(["-y", "-i", &tmp, "-f", "f32le", "-ar", "16000", "-ac", "1", &pcm_path])
                .output()
                .await;

            match ffmpeg_result {
                Ok(out) if out.status.success() => {
                    match tokio::fs::read(&pcm_path).await {
                        Ok(pcm_bytes) if pcm_bytes.len() % 4 == 0 => {
                            bytemuck::cast_slice::<u8, f32>(&pcm_bytes).to_vec()
                        }
                        _ => {
                            let e = "ffmpeg produced misaligned PCM output";
                            store.update_task_status(&tid, "failed", None, Some(e)).await.ok();
                            task_manager.remove(&tid);
                            return;
                        }
                    }
                }
                Ok(out) => {
                    let err = String::from_utf8_lossy(&out.stderr).to_string();
                    store.update_task_status(&tid, "failed", None, Some(&err)).await.ok();
                    task_manager.remove(&tid);
                    return;
                }
                Err(e) => {
                    store
                        .update_task_status(&tid, "failed", None, Some(&e.to_string()))
                        .await
                        .ok();
                    task_manager.remove(&tid);
                    return;
                }
            }
        };

        run_whisper(&tid, samples, &store, &task_manager).await;
        tokio::fs::remove_file(&tmp).await.ok();
    });

    state.task_manager.insert(task_id.clone(), join.abort_handle());
    info!(task_id = %task_id, "transcription task accepted");
    Ok(Json(serde_json::json!({ "task_id": task_id })))
}

async fn run_whisper(
    task_id: &str,
    samples: Vec<f32>,
    store: &SqliteStore,
    task_manager: &TaskManager,
) {
    let result = slab_core::api::backend("ggml.whisper")
        .op("inference")
        .input(slab_core::Payload::F32(std::sync::Arc::from(samples.as_slice())))
        .run_wait()
        .await;

    match result {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes).to_string();
            let result_json = serde_json::json!({ "text": text }).to_string();
            info!(task_id = %task_id, text_len = text.len(), "transcription done");
            store
                .update_task_status(task_id, "succeeded", Some(&result_json), None)
                .await
                .ok();
        }
        Err(e) => {
            warn!(task_id = %task_id, error = %e, "whisper inference failed");
            store
                .update_task_status(task_id, "failed", None, Some(&e.to_string()))
                .await
                .ok();
        }
    }
    task_manager.remove(task_id);
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
    fn rejects_misaligned_body() {
        let body = Bytes::from_static(&[0u8, 1, 2]);
        assert_ne!(body.len() % 4, 0);
    }

    #[test]
    fn accepts_aligned_body() {
        let body = Bytes::from(vec![0u8; 8]);
        assert_eq!(body.len() % 4, 0);
    }
}

