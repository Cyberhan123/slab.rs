//! Video generation routes.
//!
//! Generates video frames via the diffusion backend, then assembles them into
//! a video file using FFmpeg.  The result is a path to the assembled video file
//! stored in the system's temporary directory.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine as _;
use chrono::Utc;
use tracing::{debug, info, warn};
use utoipa::OpenApi;
use uuid::Uuid;

use crate::entities::{TaskRecord, TaskStore};
use crate::error::ServerError;
use crate::grpc;
use crate::schemas::v1::task::TaskResultPayload;
use crate::schemas::v1::video::VideoGenerationRequest;
use crate::state::AppState;

/// Maximum allowed prompt length in bytes.
const MAX_PROMPT_BYTES: usize = 128 * 1024;
const MAX_VIDEO_FRAMES: i32 = 120;
const MAX_IMAGE_DIM: u32 = 2048;

#[derive(OpenApi)]
#[openapi(paths(generate_video), components(schemas(VideoGenerationRequest)))]
pub struct VideoApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/video/generations", post(generate_video))
}

fn decode_init_image(data_uri: &str) -> Result<(Vec<u8>, u32, u32, u32), ServerError> {
    let b64 = if let Some(pos) = data_uri.find("base64,") {
        &data_uri[pos + "base64,".len()..]
    } else {
        data_uri
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| ServerError::BadRequest(format!("init_image base64 decode failed: {e}")))?;
    let img = image::load_from_memory(&bytes)
        .map_err(|e| ServerError::BadRequest(format!("init_image decode failed: {e}")))?;
    let rgb = img.to_rgb8();
    let (width, height) = rgb.dimensions();
    let data = rgb.into_raw();
    Ok((data, width, height, 3))
}

/// Video generation (`POST /v1/video/generations`).
///
/// Generates `video_frames` images via the diffusion backend, then assembles
/// them into an MP4 video using FFmpeg.  Returns a `task_id` immediately;
/// poll `GET /v1/tasks/{id}` until `status == "succeeded"`, then retrieve
/// the output video path from `GET /v1/tasks/{id}/result`.
#[utoipa::path(
    post,
    path = "/v1/video/generations",
    tag = "video",
    request_body = VideoGenerationRequest,
    responses(
        (status = 202, description = "Task accepted", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn generate_video(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VideoGenerationRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    // ── Validation ────────────────────────────────────────────────────────────
    if req.prompt.is_empty() {
        return Err(ServerError::BadRequest("prompt must not be empty".into()));
    }
    if req.prompt.len() > MAX_PROMPT_BYTES {
        return Err(ServerError::BadRequest(format!(
            "prompt too large ({} bytes); maximum is {} bytes",
            req.prompt.len(),
            MAX_PROMPT_BYTES
        )));
    }
    if req.video_frames < 1 || req.video_frames > MAX_VIDEO_FRAMES {
        return Err(ServerError::BadRequest(format!(
            "video_frames must be between 1 and {MAX_VIDEO_FRAMES}"
        )));
    }
    if req.width > MAX_IMAGE_DIM || req.height > MAX_IMAGE_DIM {
        return Err(ServerError::BadRequest(format!(
            "frame dimensions ({} x {}) exceed maximum of {MAX_IMAGE_DIM}",
            req.width, req.height
        )));
    }
    if !req.fps.is_finite() || req.fps < 1.0 || req.fps > 60.0 {
        return Err(ServerError::BadRequest(
            "fps must be a finite value between 1 and 60".into(),
        ));
    }

    // ── Decode optional init image ────────────────────────────────────────────
    let (init_image_bytes, init_image_width, init_image_height, init_image_channels) =
        if let Some(ref data_uri) = req.init_image {
            let (data, w, h, c) = decode_init_image(data_uri)?;
            (data, w, h, c)
        } else {
            (Vec::new(), 0u32, 0u32, 3u32)
        };

    debug!(
        model = %req.model,
        prompt_len = req.prompt.len(),
        frames = req.video_frames,
        "video generation request"
    );

    let generate_image_channel = state.grpc.generate_image_channel().ok_or_else(|| {
        ServerError::BackendNotReady("diffusion gRPC endpoint is not configured".into())
    })?;

    let task_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let fps = req.fps;
    let input_json = serde_json::json!({
        "model": req.model,
        "prompt": req.prompt,
        "negative_prompt": req.negative_prompt,
        "width": req.width,
        "height": req.height,
        "video_frames": req.video_frames,
        "fps": fps,
    });

    state
        .store
        .insert_task(TaskRecord {
            id: task_id.clone(),
            task_type: "ggml.diffusion.video".into(),
            status: "running".into(),
            model_id: None,
            input_data: Some(input_json.to_string()),
            result_data: None,
            error_msg: None,
            core_task_id: None,
            created_at: now,
            updated_at: now,
        })
        .await?;

    let grpc_req = grpc::pb::VideoRequest {
        model: req.model.clone(),
        prompt: req.prompt.clone(),
        negative_prompt: req.negative_prompt.clone().unwrap_or_default(),
        width: req.width,
        height: req.height,
        cfg_scale: req.cfg_scale.unwrap_or(7.0),
        guidance: req.guidance.unwrap_or(3.5),
        sample_steps: req.steps.unwrap_or(20),
        seed: req.seed.unwrap_or(42),
        sample_method: req.sample_method.clone().unwrap_or_default(),
        scheduler: req.scheduler.clone().unwrap_or_default(),
        video_frames: req.video_frames,
        fps: req.fps,
        strength: req.strength.unwrap_or(0.75),
        init_image_data: init_image_bytes,
        init_image_width,
        init_image_height,
        init_image_channels,
    };

    let store = Arc::clone(&state.store);
    let task_manager = Arc::clone(&state.task_manager);
    let model_auto_unload = Arc::clone(&state.model_auto_unload);
    let task_id_for_spawn = task_id.clone();

    let join = tokio::spawn(async move {
        let _usage_guard = match model_auto_unload
            .acquire_for_inference("ggml.diffusion")
            .await
        {
            Ok(guard) => guard,
            Err(error) => {
                store
                    .update_task_status(
                        &task_id_for_spawn,
                        "failed",
                        None,
                        Some(&format!("diffusion backend not ready: {error}")),
                    )
                    .await
                    .ok();
                task_manager.remove(&task_id_for_spawn);
                return;
            }
        };

        // Generate frames via the diffusion backend.
        let rpc_result = grpc::client::generate_video(generate_image_channel, grpc_req).await;

        if let Ok(Some(record)) = store.get_task(&task_id_for_spawn).await {
            if record.status == "cancelled" {
                task_manager.remove(&task_id_for_spawn);
                return;
            }
        }

        let frames_json = match rpc_result {
            Err(e) => {
                store
                    .update_task_status(&task_id_for_spawn, "failed", None, Some(&e.to_string()))
                    .await
                    .ok();
                task_manager.remove(&task_id_for_spawn);
                return;
            }
            Ok(j) => j,
        };

        // Parse frame objects: [{b64, width, height, channels}, ...]
        let frames: Vec<serde_json::Value> = match serde_json::from_slice(&frames_json) {
            Ok(v) => v,
            Err(e) => {
                store
                    .update_task_status(
                        &task_id_for_spawn,
                        "failed",
                        None,
                        Some(&format!(
                            "failed to parse frames JSON from diffusion backend: {e}"
                        )),
                    )
                    .await
                    .ok();
                task_manager.remove(&task_id_for_spawn);
                return;
            }
        };

        if frames.is_empty() {
            store
                .update_task_status(
                    &task_id_for_spawn,
                    "failed",
                    None,
                    Some("diffusion returned no frames"),
                )
                .await
                .ok();
            task_manager.remove(&task_id_for_spawn);
            return;
        }

        // Write each frame as a PNG file to a temporary directory.
        let frame_dir = std::env::temp_dir().join(format!("slab-video-{task_id_for_spawn}"));
        if let Err(e) = tokio::fs::create_dir_all(&frame_dir).await {
            store
                .update_task_status(
                    &task_id_for_spawn,
                    "failed",
                    None,
                    Some(&format!("failed to create frame dir: {e}")),
                )
                .await
                .ok();
            task_manager.remove(&task_id_for_spawn);
            return;
        }

        let mut written_index: usize = 0;
        for (i, frame) in frames.iter().enumerate() {
            let Some(b64) = frame["b64"].as_str() else {
                warn!(task_id = %task_id_for_spawn, source_frame = i, written = written_index, "frame missing b64 field; skipping");
                continue;
            };
            let frame_bytes = match base64::engine::general_purpose::STANDARD.decode(b64) {
                Ok(b) => b,
                Err(e) => {
                    warn!(task_id = %task_id_for_spawn, source_frame = i, written = written_index, error = %e, "failed to decode frame base64; skipping");
                    continue;
                }
            };
            let width = frame["width"].as_u64().unwrap_or(512) as u32;
            let height = frame["height"].as_u64().unwrap_or(512) as u32;
            let channels = frame["channels"].as_u64().unwrap_or(3) as u32;

            // Build an image from raw pixels and save as PNG.
            let img_result = if channels == 3 {
                image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(width, height, frame_bytes)
                    .map(image::DynamicImage::ImageRgb8)
            } else {
                image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, frame_bytes)
                    .map(image::DynamicImage::ImageRgba8)
            };

            let Some(img) = img_result else {
                warn!(task_id = %task_id_for_spawn, source_frame = i, written = written_index, "failed to construct image from raw pixels; skipping");
                continue;
            };

            // Use `written_index` for the filename so ffmpeg always gets a
            // contiguous sequence (frame_00000.png, frame_00001.png, …) even
            // when individual source frames are skipped above.
            let frame_path = frame_dir.join(format!("frame_{written_index:05}.png"));
            if let Err(e) = img.save(&frame_path) {
                warn!(task_id = %task_id_for_spawn, source_frame = i, written = written_index, error = %e, "failed to save frame PNG; skipping");
                continue;
            }
            written_index += 1;
        }

        if written_index == 0 {
            store
                .update_task_status(
                    &task_id_for_spawn,
                    "failed",
                    None,
                    Some("no valid frames written"),
                )
                .await
                .ok();
            task_manager.remove(&task_id_for_spawn);
            return;
        }

        // Assemble frames into an MP4 using FFmpeg.
        let output_path = std::env::temp_dir().join(format!("slab-video-{task_id_for_spawn}.mp4"));
        let frame_pattern = frame_dir.join("frame_%05d.png");
        let ffmpeg_result = tokio::process::Command::new("ffmpeg")
            .arg("-y")
            .arg("-framerate")
            .arg(fps.to_string())
            .arg("-i")
            .arg(&frame_pattern)
            .arg("-c:v")
            .arg("libx264")
            .arg("-pix_fmt")
            .arg("yuv420p")
            .arg(&output_path)
            .output()
            .await;

        // Clean up frame PNGs.
        tokio::fs::remove_dir_all(&frame_dir).await.ok();

        match ffmpeg_result {
            Ok(output) if output.status.success() => {
                let video_path = output_path.to_string_lossy().into_owned();
                info!(task_id = %task_id_for_spawn, video_path = %video_path, "video generation succeeded");
                let result = TaskResultPayload {
                    image: None,
                    images: None,
                    video_path: Some(video_path),
                    text: None,
                };
                let payload = serde_json::to_string(&result).unwrap_or_default();
                store
                    .update_task_status(&task_id_for_spawn, "succeeded", Some(&payload), None)
                    .await
                    .ok();
            }
            Ok(output) => {
                let err = String::from_utf8_lossy(&output.stderr).to_string();
                warn!(task_id = %task_id_for_spawn, error = %err, "ffmpeg failed");
                store
                    .update_task_status(&task_id_for_spawn, "failed", None, Some(&err))
                    .await
                    .ok();
            }
            Err(e) => {
                warn!(task_id = %task_id_for_spawn, error = %e, "ffmpeg spawn failed");
                store
                    .update_task_status(&task_id_for_spawn, "failed", None, Some(&e.to_string()))
                    .await
                    .ok();
            }
        }
        task_manager.remove(&task_id_for_spawn);
    });

    state
        .task_manager
        .insert(task_id.clone(), join.abort_handle());

    Ok(Json(serde_json::json!({ "task_id": task_id })))
}
