//! Video generation routes.
//!
//! Generates video frames through the diffusion backend and assembles them into
//! an MP4 file with FFmpeg.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine as _;
use tracing::{debug, info, warn};
use utoipa::OpenApi;

use crate::context::{AppState, SubmitOperation, WorkerState};
use crate::error::ServerError;
use crate::infra::rpc::{self, pb};
use crate::schemas::v1::task::{OperationAcceptedResponse, TaskResultPayload};
use crate::schemas::v1::video::VideoGenerationRequest;

const MAX_PROMPT_BYTES: usize = 128 * 1024;
const MAX_VIDEO_FRAMES: i32 = 120;
const MAX_IMAGE_DIM: u32 = 2048;

#[derive(OpenApi)]
#[openapi(
    paths(generate_video),
    components(schemas(VideoGenerationRequest, OperationAcceptedResponse))
)]
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

#[utoipa::path(
    post,
    path = "/v1/video/generations",
    tag = "video",
    request_body = VideoGenerationRequest,
    responses(
        (status = 202, description = "Task accepted", body = OperationAcceptedResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn generate_video(
    State(worker_state): State<WorkerState>,
    Json(req): Json<VideoGenerationRequest>,
) -> Result<(StatusCode, Json<OperationAcceptedResponse>), ServerError> {
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

    let generate_image_channel = worker_state.grpc().generate_image_channel().ok_or_else(|| {
        ServerError::BackendNotReady("diffusion gRPC endpoint is not configured".into())
    })?;

    let fps = req.fps;
    let input_json = serde_json::json!({
        "model": req.model,
        "prompt": req.prompt,
        "negative_prompt": req.negative_prompt,
        "width": req.width,
        "height": req.height,
        "video_frames": req.video_frames,
        "fps": fps,
    })
    .to_string();

    let grpc_req = pb::VideoRequest {
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

    let model_auto_unload = Arc::clone(worker_state.auto_unload());
    let operation_id = worker_state
        .submit_operation(
            SubmitOperation::running("ggml.diffusion.video", None, Some(input_json)),
            move |operation| async move {
                let operation_id = operation.id().to_owned();

                let _usage_guard = match model_auto_unload.acquire_for_inference("ggml.diffusion").await {
                    Ok(guard) => guard,
                    Err(error) => {
                        let msg = format!("diffusion backend not ready: {error}");
                        if let Err(db_e) = operation.mark_failed(&msg).await {
                            warn!(task_id = %operation_id, error = %db_e, "failed to persist backend-not-ready error");
                        }
                        return;
                    }
                };

                let rpc_result = rpc::client::generate_video(generate_image_channel, grpc_req).await;
                if operation.is_cancelled().await {
                    return;
                }

                let frames_json = match rpc_result {
                    Ok(payload) => payload,
                    Err(e) => {
                        if let Err(db_e) = operation.mark_failed(&e.to_string()).await {
                            warn!(task_id = %operation_id, error = %db_e, "failed to persist diffusion video error");
                        }
                        return;
                    }
                };

                let frames: Vec<serde_json::Value> = match serde_json::from_slice(&frames_json) {
                    Ok(v) => v,
                    Err(e) => {
                        let msg = format!("failed to parse frames JSON from diffusion backend: {e}");
                        if let Err(db_e) = operation.mark_failed(&msg).await {
                            warn!(task_id = %operation_id, error = %db_e, "failed to persist frame parse error");
                        }
                        return;
                    }
                };

                if frames.is_empty() {
                    if let Err(db_e) = operation.mark_failed("diffusion returned no frames").await {
                        warn!(task_id = %operation_id, error = %db_e, "failed to persist empty-frame error");
                    }
                    return;
                }

                let frame_dir = std::env::temp_dir().join(format!("slab-video-{operation_id}"));
                if let Err(e) = tokio::fs::create_dir_all(&frame_dir).await {
                    let msg = format!("failed to create frame dir: {e}");
                    if let Err(db_e) = operation.mark_failed(&msg).await {
                        warn!(task_id = %operation_id, error = %db_e, "failed to persist frame-dir error");
                    }
                    return;
                }

                let mut written_index: usize = 0;
                for (i, frame) in frames.iter().enumerate() {
                    let Some(b64) = frame["b64"].as_str() else {
                        warn!(task_id = %operation_id, source_frame = i, written = written_index, "frame missing b64 field; skipping");
                        continue;
                    };
                    let frame_bytes = match base64::engine::general_purpose::STANDARD.decode(b64) {
                        Ok(b) => b,
                        Err(e) => {
                            warn!(task_id = %operation_id, source_frame = i, written = written_index, error = %e, "failed to decode frame base64; skipping");
                            continue;
                        }
                    };
                    let width = frame["width"].as_u64().unwrap_or(512) as u32;
                    let height = frame["height"].as_u64().unwrap_or(512) as u32;
                    let channels = frame["channels"].as_u64().unwrap_or(3) as u32;

                    let img_result = if channels == 3 {
                        image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(width, height, frame_bytes)
                            .map(image::DynamicImage::ImageRgb8)
                    } else {
                        image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, frame_bytes)
                            .map(image::DynamicImage::ImageRgba8)
                    };

                    let Some(img) = img_result else {
                        warn!(task_id = %operation_id, source_frame = i, written = written_index, "failed to construct image from raw pixels; skipping");
                        continue;
                    };

                    let frame_path = frame_dir.join(format!("frame_{written_index:05}.png"));
                    if let Err(e) = img.save(&frame_path) {
                        warn!(task_id = %operation_id, source_frame = i, written = written_index, error = %e, "failed to save frame PNG; skipping");
                        continue;
                    }
                    written_index += 1;
                }

                if written_index == 0 {
                    if let Err(db_e) = operation.mark_failed("no valid frames written").await {
                        warn!(task_id = %operation_id, error = %db_e, "failed to persist no-valid-frame error");
                    }
                    return;
                }

                let output_path = std::env::temp_dir().join(format!("slab-video-{operation_id}.mp4"));
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

                tokio::fs::remove_dir_all(&frame_dir).await.ok();

                match ffmpeg_result {
                    Ok(output) if output.status.success() => {
                        let video_path = output_path.to_string_lossy().into_owned();
                        info!(task_id = %operation_id, video_path = %video_path, "video generation succeeded");
                        let result = TaskResultPayload {
                            image: None,
                            images: None,
                            video_path: Some(video_path),
                            text: None,
                        };
                        let payload = serde_json::to_string(&result).unwrap_or_default();
                        if let Err(db_e) = operation.mark_succeeded(&payload).await {
                            warn!(task_id = %operation_id, error = %db_e, "failed to persist video task success");
                        }
                    }
                    Ok(output) => {
                        let err = String::from_utf8_lossy(&output.stderr).to_string();
                        warn!(task_id = %operation_id, error = %err, "ffmpeg failed");
                        if let Err(db_e) = operation.mark_failed(&err).await {
                            warn!(task_id = %operation_id, error = %db_e, "failed to persist ffmpeg failure");
                        }
                    }
                    Err(e) => {
                        warn!(task_id = %operation_id, error = %e, "ffmpeg spawn failed");
                        if let Err(db_e) = operation.mark_failed(&e.to_string()).await {
                            warn!(task_id = %operation_id, error = %db_e, "failed to persist ffmpeg spawn failure");
                        }
                    }
                }
            },
        )
        .await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(OperationAcceptedResponse { operation_id }),
    ))
}
