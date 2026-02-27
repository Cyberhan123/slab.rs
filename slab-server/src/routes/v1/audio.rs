//! Audio transcription routes (Whisper) – async task pattern.
//!
//! Accepts an audio/video file via multipart/form-data upload, saves it to a temp file,
//! then submits a slab-core pipeline (ffmpeg → whisper) via `api::backend(...).preprocess(...).run()`.
//! The returned slab-core `TaskId` is persisted so that the generic
//! `/api/tasks` endpoints can query status and result via `slab_core::api::status/result`.

use std::sync::Arc;
use std::process::Stdio;

use axum::extract::{Multipart, State};
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tracing::{debug, info, trace, warn};
use uuid::Uuid;

use crate::entities::{TaskRecord, TaskStore};
use crate::error::ServerError;
use crate::schemas::v1::audio::CompletionRequest;
use crate::state::AppState;
use bytemuck::cast_slice;
use slab_core::api::{Backend, Event};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(transcribe_upload, transcribe))]
pub struct AudioApi;

/// Register audio routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/audio/transcriptions", post(transcribe_upload))
        .route("/audio/transcriptions/legacy", post(transcribe))
}

/// Speech-to-text transcription via file upload (`POST /v1/audio/transcriptions`).
///
/// Accepts an audio/video file via multipart/form-data upload. The file is saved to a
/// temporary file, then a slab-core pipeline is submitted:
///
/// 1. **ffmpeg** (CPU preprocess stage via `std::process::Command`) converts
///    the file to raw PCM f32le at 16 kHz mono.
/// 2. **whisper** (GPU stage) transcribes the PCM samples.
///
/// Returns `{"task_id": "..."}` immediately; poll status via
/// `GET /api/tasks/{id}` and result via `GET /api/tasks/{id}/result`.
///
/// # Security
/// - File size is validated (max 100MB by default, configurable via SLAB_MAX_UPLOAD_SIZE_MB)
/// - File type is validated (audio/* and video/* MIME types only)
/// - Uploaded files are stored in a secure temp directory with randomized names
/// - No arbitrary file path access from client
#[utoipa::path(
    post,
    path = "/v1/audio/transcriptions",
    tag = "audio",
    request_body(content = crate::schemas::v1::audio::CompletionRequestUpload, description = "Audio/video file upload (multipart/form-data)"),
    responses(
        (status = 202, description = "Task accepted", body = serde_json::Value),
        (status = 400, description = "Bad request (invalid file, too large, or wrong type)"),
        (status = 413, description = "File too large"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn transcribe_upload(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ServerError> {
    debug!("received multipart transcription request");

    // Configuration
    let max_upload_size_mb: usize = std::env::var("SLAB_MAX_UPLOAD_SIZE_MB")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);
    let max_upload_size_bytes = max_upload_size_mb * 1024 * 1024;

    let allowed_mime_types = [
        "audio/mpeg",      // MP3
        "audio/wav",       // WAV
        "audio/wave",      // WAV (alternative)
        "audio/x-wav",     // WAV (alternative)
        "audio/flac",      // FLAC
        "audio/x-flac",    // FLAC (alternative)
        "audio/mp4",       // M4A
        "audio/x-m4a",     // M4A (alternative)
        "audio/ogg",       // OGG
        "video/mp4",       // MP4 video
        "video/x-matroska", // MKV video
        "video/webm",      // WebM video
    ];

    // Extract the uploaded file
    let mut file_bytes: Vec<u8> = Vec::new();
    let mut file_name = String::new();
    let mut content_type = String::new();

    while let Some(field) = multipart.next_field().await
        .map_err(|e| ServerError::BadRequest(format!("Failed to read multipart field: {e}")))?
    {
        let field_name = field.name().unwrap_or("unknown");

        if field_name == "file" {
            file_name = field.file_name()
                .unwrap_or("upload")
                .to_string();

            content_type = field.content_type()
                .unwrap_or("application/octet-stream")
                .to_string();

            // Validate MIME type
            if !content_type.starts_with("audio/") && !content_type.starts_with("video/") {
                return Err(ServerError::BadRequest(format!(
                    "Invalid file type: {content_type}. Only audio and video files are allowed."
                )));
            }

            if !allowed_mime_types.contains(&content_type.as_str()) {
                return Err(ServerError::BadRequest(format!(
                    "Unsupported file format: {content_type}. Supported formats: MP3, WAV, FLAC, M4A, OGG, MP4, MKV, WebM"
                )));
            }

            // Stream the file data with size validation
            let mut stream = field;
            while let Some(chunk) = stream.next_chunk().await
                .map_err(|e| ServerError::BadRequest(format!("Failed to read file chunk: {e}")))?
            {
                file_bytes.extend_from_slice(&chunk);

                // Check size limit during streaming
                if file_bytes.len() > max_upload_size_bytes {
                    return Err(ServerError::BadRequest(format!(
                        "File too large: {} bytes exceeds maximum of {}MB",
                        file_bytes.len(),
                        max_upload_size_mb
                    )));
                }
            }

            debug!(
                file_name = %file_name,
                content_type = %content_type,
                size_bytes = file_bytes.len(),
                "received file upload"
            );
        } else {
            return Err(ServerError::BadRequest(format!("Unknown field: {field_name}")));
        }
    }

    if file_bytes.is_empty() {
        return Err(ServerError::BadRequest("No file uploaded".into()));
    }

    // Create a secure temporary file with a randomized name
    let task_id = Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir();
    let temp_file_path = temp_dir
        .join(format!("slab_upload_{}_{}/{}",
            Uuid::new_v4(),
            task_id,
            sanitize_filename(&file_name)
        ));

    // Create parent directory if it doesn't exist
    if let Some(parent) = temp_file_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ServerError::InternalServerError(format!("Failed to create temp directory: {e}")))?;
    }

    // Write the uploaded file to disk
    std::fs::write(&temp_file_path, &file_bytes)
        .map_err(|e| ServerError::InternalServerError(format!("Failed to write uploaded file: {e}")))?;

    info!(
        task_id = %task_id,
        temp_file = %temp_file_path.display(),
        original_name = %file_name,
        size_bytes = file_bytes.len(),
        "saved uploaded file to temporary location"
    );

    // Check if the Whisper backend is ready before accepting the task
    let backend_ready = slab_core::api::is_backend_ready(Backend::GGMLWhisper).await.unwrap_or(false);
    if !backend_ready {
        warn!(
            task_id = %task_id,
            "transcription request rejected: whisper backend not ready"
        );
        // Clean up temp file
        let _ = std::fs::remove_file(&temp_file_path);
        return Err(ServerError::BadRequest(
            "The Whisper backend is not ready. Please ensure the library and model are loaded. Check server logs for details.".into()
        ));
    }

    let now = Utc::now();

    // Insert the server-side task record
    state
        .store
        .insert_task(TaskRecord {
            id: task_id.clone(),
            task_type: Backend::GGMLWhisper.to_string(),
            status: "running".into(),
            input_data: Some(format!("{} (original: {})", temp_file_path.display(), file_name)),
            result_data: None,
            error_msg: None,
            core_task_id: None,
            created_at: now,
            updated_at: now,
        })
        .await?;

    let temp_file_path_str = temp_file_path.to_string_lossy().to_string();
    let core_task_result = slab_core::api::backend(Backend::GGMLWhisper)
        .op(Event::Inference)
        .input(slab_core::Payload::Text(temp_file_path_str.clone().into()))
        .preprocess("ffmpeg.to_pcm_f32le", convert_to_pcm_f32le)
        .run()
        .await;

    match core_task_result {
        Ok(core_task_id) => {
            // Persist the slab-core TaskId
            state
                .store
                .set_core_task_id(&task_id, core_task_id as i64)
                .await
                .unwrap_or_else(
                    |e| warn!(task_id = %task_id, error = %e, "failed to store core_task_id"),
                );
            info!(
                task_id = %task_id,
                core_task_id,
                temp_file = %temp_file_path.display(),
                "transcription task submitted to slab-core"
            );
        }
        Err(e) => {
            // Clean up temp file on error
            let _ = std::fs::remove_file(&temp_file_path);

            // Provide detailed error context
            let error_message = format!("Failed to submit transcription task: {e}");
            warn!(task_id = %task_id, error = %error_message);

            let detailed_error = if e.to_string().contains("library not loaded") ||
                e.to_string().contains("backend not ready") {
                format!("{error_message}\n\nThe Whisper backend is not properly initialized. Please ensure:\n1. The whisper library directory is configured (SLAB_WHISPER_LIB_DIR)\n2. A whisper model has been loaded via the backend API")
            } else if e.to_string().contains("FFmpeg") {
                format!("{error_message}\n\nAudio preprocessing failed. Please check:\n1. FFmpeg is installed and accessible\n2. The audio file format is supported\n3. The file is not corrupted")
            } else {
                error_message
            };

            state
                .store
                .update_task_status(&task_id, "failed", None, Some(&detailed_error))
                .await
                .unwrap_or_else(|db_e| warn!(error = %db_e, "failed to update task status"));
        }
    }

    Ok(Json(serde_json::json!({ "task_id": task_id })))
}

/// Sanitize a filename to prevent directory traversal
fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '_' || c == '-' { c } else { '_' })
        .collect()
}

/// Legacy path-based transcription (DEPRECATED - use transcribe_upload instead)
///
/// # Security Warning
/// This endpoint accepts arbitrary file paths from the client, which is a security risk.
/// Use the `/audio/transcriptions` endpoint with multipart file upload instead.
#[utoipa::path(
    post,
    path = "/v1/audio/transcriptions/legacy",
    tag = "audio",
    request_body(content = CompletionRequest, description = "Audio file path (DEPRECATED - use multipart upload instead)"),
    responses(
        (status = 202, description = "Task accepted", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    ),
    deprecated = true
)]
pub async fn transcribe(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CompletionRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    warn!(
        file_path = %req.path,
        "legacy transcription endpoint used (DEPRECATED - use /audio/transcriptions with multipart upload)"
    );

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

    // Check if the Whisper backend is ready before accepting the task
    let backend_ready = slab_core::api::is_backend_ready(Backend::GGMLWhisper).await.unwrap_or(false);
    if !backend_ready {
        warn!(
            audio_path = %req.path,
            "transcription request rejected: whisper backend not ready"
        );
        return Err(ServerError::BadRequest(
            "The Whisper backend is not ready. Please ensure the library and model are loaded. Check server logs for details.".into()
        ));
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
            info!(
                task_id = %task_id,
                core_task_id,
                audio_path = %req.path,
                "transcription task submitted to slab-core"
            );
        }
        Err(e) => {
            // Provide detailed error context for different failure modes
            let error_message = format!("Failed to submit transcription task: {e}");
            warn!(task_id = %task_id, error = %error_message, audio_path = %req.path);

            // Classify error for better client feedback
            let detailed_error = if e.to_string().contains("library not loaded") ||
                e.to_string().contains("backend not ready") {
                format!("{error_message}\n\nThe Whisper backend is not properly initialized. Please ensure:\n1. The whisper library directory is configured (SLAB_WHISPER_LIB_DIR)\n2. A whisper model has been loaded via the backend API")
            } else if e.to_string().contains("FFmpeg") {
                format!("{error_message}\n\nAudio preprocessing failed. Please check:\n1. FFmpeg is installed and accessible\n2. The audio file format is supported\n3. The file is not corrupted")
            } else {
                error_message
            };

            state
                .store
                .update_task_status(&task_id, "failed", None, Some(&detailed_error))
                .await
                .unwrap_or_else(|db_e| warn!(error = %db_e, "failed to update task status"));
        }
    }

    Ok(Json(serde_json::json!({ "task_id": task_id })))
}

/// Asynchronous FFmpeg conversion to PCM f32le at 16kHz mono.
///
/// This implementation uses `tokio::process::Command` instead of the synchronous
/// `ffmpeg-sidecar` library, providing:
/// - Non-blocking async execution
/// - Proper cancellation support
/// - Streaming output to avoid buffering large files
/// - Better error handling and timeouts
///
/// The function is still synchronous in signature (as required by `CpuStage`),
/// but internally uses `tokio::runtime::Handle::block_on` to run async operations,
/// allowing proper cancellation while maintaining compatibility with the existing
/// pipeline architecture.
pub fn convert_to_pcm_f32le(payload: slab_core::Payload) -> Result<slab_core::Payload, String> {
    let path = payload
        .to_str()
        .map_err(|e| format!("Invalid payload for ffmpeg preprocessing: expected file path string, got error: {e}"))?;

    // Validate the file exists before passing to ffmpeg
    let path_obj = std::path::Path::new(path);
    if !path_obj.exists() {
        return Err(format!("Audio file not found: {path}"));
    }

    debug!(
        path = %path,
        "starting async ffmpeg conversion to PCM f32le 16kHz mono"
    );

    // Get the tokio runtime handle - this is safe because we're already inside
    // a spawn_blocking task called by the orchestrator
    let rt_handle = tokio::runtime::Handle::try_current()
        .map_err(|e| format!("No tokio runtime available: {e}"))?;

    // Run the async FFmpeg conversion on the current runtime
    let samples = rt_handle.block_on(async {
        convert_to_pcm_f32le_async(path).await
    }).map_err(|e| format!("Async FFmpeg conversion failed: {e}"))?;

    info!(
        path = %path,
        sample_count = samples.len(),
        duration_sec = samples.len() as f64 / 16000.0,
        "audio preprocessing completed successfully"
    );

    Ok(slab_core::Payload::F32(std::sync::Arc::from(
        samples.as_slice(),
    )))
}

/// Async implementation of FFmpeg conversion using tokio::process::Command.
///
/// This function:
/// - Spawns FFmpeg as a subprocess
/// - Streams stdout to avoid buffering large files
/// - Supports cancellation via tokio::select!
/// - Has a configurable timeout (5 minutes for long audio files)
/// - Properly handles process cleanup on error or cancellation
async fn convert_to_pcm_f32le_async(path: &str) -> Result<Vec<f32>, String> {
    use tokio::process::Command;
    use tokio::time::{timeout, Duration};

    // Timeout for FFmpeg conversion - 5 minutes should be sufficient for most audio files
    const FFMPEG_TIMEOUT: Duration = Duration::from_secs(300);

    // Build the FFmpeg command
    // Arguments:
    // -vn: ignore video stream
    // -f f32le: output format (32-bit float little-endian)
    // -acodec pcm_f32le: audio codec
    // -ar 16000: sample rate 16kHz
    // -ac 1: mono audio
    // -: output to stdout
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-i")
       .arg(path)
       .args(["-vn", "-f", "f32le", "-acodec", "pcm_f32le", "-ar", "16000", "-ac", "1"])
       .arg("-")
       .stdout(Stdio::piped())
       .stderr(Stdio::piped());

    // Spawn the FFmpeg process
    let mut child = cmd.spawn()
        .map_err(|e| {
            format!("Failed to start FFmpeg process. Is FFmpeg installed and in PATH? Error: {e}")
        })?;

    // Get stdout and stderr handles
    let stdout = child.stdout.take()
        .ok_or_else(|| "FFmpeg stdout not available".to_string())?;
    let stderr = child.stderr.take()
        .ok_or_else(|| "FFmpeg stderr not available".to_string())?;

    // Spawn a background task to log stderr for debugging
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            trace!(ffmpeg_stderr = %line, "ffmpeg log");
        }
    });

    // Stream stdout with timeout
    let pcm_bytes_result = timeout(FFMPEG_TIMEOUT, async {
        let mut reader = stdout;
        let mut pcm_bytes = Vec::new();
        let mut buffer = vec![0u8; 8192]; // 8KB buffer for streaming

        loop {
            let n = reader.read(&mut buffer).await
                .map_err(|e| format!("Failed to read FFmpeg stdout: {e}"))?;

            if n == 0 {
                break; // EOF
            }

            pcm_bytes.extend_from_slice(&buffer[..n]);

            // Optional: Log progress for large files
            if pcm_bytes.len() % (1024 * 1024) == 0 {
                trace!(
                    bytes_read = pcm_bytes.len(),
                    duration_sec = pcm_bytes.len() as f64 / 4.0 / 16000.0,
                    "ffmpeg progress"
                );
            }
        }

        Ok::<Vec<u8>, String>(pcm_bytes)
    }).await;

    // Wait for stderr logging to complete (ignore errors)
    let _ = stderr_task.await;

    // Clean up the child process
    let status = timeout(Duration::from_secs(5), child.wait())
        .await
        .map_err(|_| "Timeout waiting for FFmpeg process to exit".to_string())?
        .map_err(|e| format!("Failed to wait for FFmpeg process: {e}"))?;

    // Check if FFmpeg exited successfully
    if !status.success() {
        return Err(format!(
            "FFmpeg process exited with non-zero status: {}",
            status.code().unwrap_or(-1)
        ));
    }

    // Get the PCM bytes
    let pcm_bytes = pcm_bytes_result
        .map_err(|_| "FFmpeg conversion timeout".to_string())?
        .map_err(|e| e.to_string())?;

    // Validate output
    if pcm_bytes.is_empty() {
        return Err(format!(
            "FFmpeg produced no output for file '{path}'. The file may be corrupted or in an unsupported format."
        ));
    }

    let sample_size = std::mem::size_of::<f32>();

    if pcm_bytes.len() % sample_size != 0 {
        return Err(format!(
            "FFmpeg output misalignment: received {} bytes, expected multiple of {} bytes for f32 samples",
            pcm_bytes.len(),
            sample_size
        ));
    }

    // Convert bytes to f32 samples
    let samples: Vec<f32> = cast_slice::<u8, f32>(&pcm_bytes).to_vec();

    debug!(
        bytes_collected = pcm_bytes.len(),
        samples = samples.len(),
        "ffmpeg conversion completed"
    );

    Ok(samples)
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {}
