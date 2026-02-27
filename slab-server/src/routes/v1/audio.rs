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
use chrono::Utc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::entities::{TaskRecord, TaskStore};
use crate::error::ServerError;
use crate::schemas::v1::audio::CompletionRequest;
use crate::state::AppState;
use bytemuck::cast_slice;
use ffmpeg_sidecar::{
    command::FfmpegCommand, event::FfmpegEvent,
};
use slab_core::api::{Backend, Event};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(transcribe))]
pub struct AudioApi;

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
    request_body(content = CompletionRequest, description = "Audio/video file bytes"),
    responses(
        (status = 202, description = "Task accepted", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn transcribe(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CompletionRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    debug!(file_path = %req.path, "transcription request");

    if req.path.is_empty() {
        return Err(ServerError::BadRequest("audio file path is empty".into()));
    }

    // Validate file exists and is readable
    let path = std::path::Path::new(&req.path);
    if !path.exists() {
        return Err(ServerError::BadRequest(
            format!("Audio file does not exist: {}", req.path)
        ));
    }

    if !path.is_file() {
        return Err(ServerError::BadRequest(
            format!("Path is not a file: {}", req.path)
        ));
    }

    // Check file permissions (readable)
    match std::fs::metadata(&req.path) {
        Ok(metadata) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = metadata.permissions().mode();
                if mode & 0o444 == 0 {
                    return Err(ServerError::BadRequest(
                        format!("Audio file is not readable: {}", req.path)
                    ));
                }
            }
        }
        Err(e) => {
            return Err(ServerError::BadRequest(
                format!("Cannot access audio file: {} - {}", req.path, e)
            ));
        }
    }

    let task_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    // Insert the server-side task record (core_task_id filled in after submission).
    state
        .store
        .insert_task(TaskRecord {
            id: task_id.clone(),
            task_type: Backend::GGMLWhisper.to_string(),
            status: "running".into(),
            input_data: Some(req.path.clone()),
            result_data: None,
            error_msg: None,
            core_task_id: None,
            created_at: now,
            updated_at: now,
        })
        .await?;

    let core_task_result = slab_core::api::backend(Backend::GGMLWhisper)
        .op(Event::Inference)
        .input(slab_core::Payload::Text(req.path.clone().into())) 
        .preprocess("ffmpeg.to_pcm_f32le", convert_to_pcm_f32le)
        .run()
        .await;

    match core_task_result {
        Ok(core_task_id) => {
            // Persist the slab-core TaskId so status/result queries can use it.
            state
                .store
                .set_core_task_id(&task_id, core_task_id as i64)
                .await
                .unwrap_or_else(
                    |e| warn!(task_id = %task_id, error = %e, "failed to store core_task_id"),
                );
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

    Ok(Json(serde_json::json!({ "task_id": task_id })))
}

pub fn convert_to_pcm_f32le(payload: slab_core::Payload) -> Result<slab_core::Payload, String> {
    let path = payload
        .to_str()
        .map_err(|e| format!("Invalid payload for preprocess: {e}"))?;
    let iter = FfmpegCommand::new()
        .input(path)
        .args([
            "-vn",
            "-f",
            "f32le",
            "-acodec",
            "pcm_f32le",
            "-ar",
            "16000",
            "-ac",
            "1",
        ])
        .output("-")
        .spawn()
        .map_err(|e| format!("FFmpeg start failed: {e}"))?
        .iter()
        .map_err(|e| format!("FFmpeg start failed: {e}"))?;

    let mut pcm_bytes = Vec::new();

    for event in iter {
        match event {
            FfmpegEvent::OutputChunk(chunk) => {
                pcm_bytes.extend_from_slice(&chunk);
            }
            FfmpegEvent::Done => break,
            FfmpegEvent::Error(e) => return Err(format!("FFmpeg fail on run: {e}")),
            _ => {}
        }
    }

    let sample_size = std::mem::size_of::<f32>();

    if pcm_bytes.len() % sample_size != 0 {
        return Err(format!("PCM not aligned: {} bytes", pcm_bytes.len()));
    }

    let samples: Vec<f32> = cast_slice::<u8, f32>(&pcm_bytes).to_vec();

    Ok(slab_core::Payload::F32(std::sync::Arc::from(
        samples.as_slice(),
    )))
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {}
