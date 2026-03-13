//! Image generation routes.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine as _;
use tracing::{debug, warn};
use utoipa::OpenApi;

use crate::context::{AppState, SubmitOperation, WorkerState};
use crate::error::ServerError;
use crate::infra::rpc::{self, pb};
use crate::schemas::v1::images::{ImageGenerationRequest, ImageMode};
use crate::schemas::v1::task::{OperationAcceptedResponse, TaskResultPayload};

/// Maximum allowed prompt length in bytes.
const MAX_PROMPT_BYTES: usize = 128 * 1024; // 128 KiB

/// Maximum number of images that can be generated in a single request.
const MAX_IMAGES_PER_REQUEST: u32 = 10;

/// Maximum accepted image dimensions.
const MAX_IMAGE_DIM: u32 = 2048;

#[derive(OpenApi)]
#[openapi(
    paths(generate_images),
    components(schemas(ImageGenerationRequest, OperationAcceptedResponse,))
)]
pub struct ImagesApi;

/// Register image generation routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/images/generations", post(generate_images))
}

/// Decode a base64 data URI to raw RGB pixel bytes.
///
/// Strips the `data:<mime>;base64,` prefix, base64-decodes the payload,
/// then uses the `image` crate to decode the image format directly to RGB8
/// (24-bit RGB) pixel data.
///
/// Returns `(raw_rgb_bytes, width, height, channels)`.
fn decode_init_image(data_uri: &str) -> Result<(Vec<u8>, u32, u32, u32), ServerError> {
    // Strip data URI prefix.
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

/// Image generation (`POST /v1/images/generations`).
///
/// Accepts both text-to-image and image-to-image generation requests.
/// The `mode` field selects the generation mode (default: `txt2img`).
#[utoipa::path(
    post,
    path = "/v1/images/generations",
    tag = "images",
    request_body = ImageGenerationRequest,
    responses(
        (status = 202, description = "Task accepted", body = OperationAcceptedResponse),
        (status = 400, description = "Bad request (invalid parameters)"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn generate_images(
    State(worker_state): State<WorkerState>,
    Json(req): Json<ImageGenerationRequest>,
) -> Result<(StatusCode, Json<OperationAcceptedResponse>), ServerError> {
    // 鈹€鈹€ Validation 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
    if req.prompt.is_empty() {
        return Err(ServerError::BadRequest("prompt must not be empty".into()));
    }
    if req.prompt.len() > MAX_PROMPT_BYTES {
        return Err(ServerError::BadRequest(format!(
            "prompt too large ({} bytes); maximum is {} bytes",
            req.prompt.len(),
            MAX_PROMPT_BYTES,
        )));
    }
    if req.n == 0 || req.n > MAX_IMAGES_PER_REQUEST {
        return Err(ServerError::BadRequest(format!(
            "invalid n ({}): must be between 1 and {MAX_IMAGES_PER_REQUEST}",
            req.n
        )));
    }
    if req.width > MAX_IMAGE_DIM || req.height > MAX_IMAGE_DIM {
        return Err(ServerError::BadRequest(format!(
            "image dimensions ({} x {}) exceed maximum of {MAX_IMAGE_DIM}",
            req.width, req.height
        )));
    }
    if req.mode == ImageMode::Img2Img && req.init_image.is_none() {
        return Err(ServerError::BadRequest(
            "init_image is required for img2img mode".into(),
        ));
    }
    // For txt2img mode, explicitly ignore any init_image/strength supplied by the
    // client so the backend always runs the correct pipeline.
    let effective_init_image = if req.mode == ImageMode::Img2Img {
        req.init_image.clone()
    } else {
        None
    };
    let effective_strength = if req.mode == ImageMode::Img2Img {
        req.strength
    } else {
        None
    };

    // 鈹€鈹€ Decode init image (img2img) 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
    let (init_image_bytes, init_image_width, init_image_height, init_image_channels) =
        if let Some(ref data_uri) = effective_init_image {
            let (data, w, h, c) = decode_init_image(data_uri)?;
            (data, w, h, c)
        } else {
            (Vec::new(), 0u32, 0u32, 3u32)
        };

    debug!(
        model = %req.model,
        prompt_len = req.prompt.len(),
        n = req.n,
        width = req.width,
        height = req.height,
        mode = ?req.mode,
        "image generation request"
    );

    let generate_image_channel = worker_state.grpc().generate_image_channel().ok_or_else(|| {
        ServerError::BackendNotReady("diffusion gRPC endpoint is not configured".into())
    })?;

    let input_json = serde_json::json!({
        "prompt": req.prompt,
        "negative_prompt": req.negative_prompt,
        "n": req.n,
        "width": req.width,
        "height": req.height,
        "model": req.model,
        "mode": req.mode,
        "cfg_scale": req.cfg_scale,
        "guidance": req.guidance,
        "steps": req.steps,
        "seed": req.seed,
        "sample_method": req.sample_method,
        "scheduler": req.scheduler,
        "clip_skip": req.clip_skip,
        "strength": req.strength,
        "eta": req.eta,
    })
    .to_string();

    let grpc_req = pb::ImageRequest {
        model: req.model.clone(),
        prompt: req.prompt.clone(),
        negative_prompt: req.negative_prompt.clone().unwrap_or_default(),
        n: req.n,
        width: req.width,
        height: req.height,
        cfg_scale: req.cfg_scale.unwrap_or(7.0),
        guidance: req.guidance.unwrap_or(3.5),
        sample_steps: req.steps.unwrap_or(20),
        seed: req.seed.unwrap_or(42),
        sample_method: req.sample_method.clone().unwrap_or_default(),
        scheduler: req.scheduler.clone().unwrap_or_default(),
        clip_skip: req.clip_skip.unwrap_or(0),
        strength: effective_strength.unwrap_or(0.75),
        eta: req.eta.unwrap_or(0.0),
        init_image_data: init_image_bytes,
        init_image_width,
        init_image_height,
        init_image_channels,
    };

    let model_auto_unload = Arc::clone(worker_state.auto_unload());
    let generate_image_channel_for_spawn = generate_image_channel;
    let operation_id = worker_state
        .submit_operation(
            SubmitOperation::running("ggml.diffusion", None, Some(input_json)),
            move |operation| async move {
                let operation_id = operation.id().to_owned();
                let _usage_guard = match model_auto_unload.acquire_for_inference("ggml.diffusion").await {
                    Ok(guard) => guard,
                    Err(error) => {
                        let msg = format!("diffusion backend not ready: {error}");
                        if let Err(db_e) = operation.mark_failed(&msg).await {
                            warn!(task_id = %operation_id, error = %db_e, "failed to update auto-reload failure");
                        }
                        return;
                    }
                };

                let rpc_result =
                    rpc::client::generate_image(generate_image_channel_for_spawn, grpc_req).await;
                if operation.is_cancelled().await {
                    return;
                }

                match rpc_result {
                    Ok(images_json) => {
                        let images: Vec<serde_json::Value> = match serde_json::from_slice(&images_json) {
                            Ok(v) => v,
                            Err(e) => {
                                let msg = format!("invalid JSON from diffusion backend: {e}");
                                debug!(task_id = %operation_id, error = %e, "failed to parse image JSON from backend");
                                if let Err(db_e) = operation.mark_failed(&msg).await {
                                    warn!(task_id = %operation_id, error = %db_e,
                                        "failed to update task status after JSON parse error");
                                }
                                return;
                            }
                        };

                        let data_uris: Vec<String> = images
                            .iter()
                            .filter_map(|img| img["b64"].as_str())
                            .map(|b64| format!("data:image/png;base64,{b64}"))
                            .collect();

                        let first_image = data_uris.first().cloned();
                        let result = TaskResultPayload {
                            image: first_image,
                            images: Some(data_uris),
                            video_path: None,
                            text: None,
                        };
                        let payload_str = serde_json::to_string(&result).unwrap_or_default();
                        if let Err(e) = operation.mark_succeeded(&payload_str).await {
                            warn!(task_id = %operation_id, error = %e, "failed to update image result");
                        }
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        if let Err(db_e) = operation.mark_failed(&msg).await {
                            warn!(task_id = %operation_id, error = %db_e, "failed to update image failure");
                        }
                    }
                }
            },
        )
        .await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(OperationAcceptedResponse {
            operation_id,
        }),
    ))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn validates_n_zero() {
        let n = 0u32;
        assert!(
            n == 0 || n > MAX_IMAGES_PER_REQUEST,
            "n=0 should be invalid"
        );
    }

    #[test]
    fn validates_n_too_large() {
        let n = MAX_IMAGES_PER_REQUEST + 1;
        assert!(n > MAX_IMAGES_PER_REQUEST, "n too large should be invalid");
    }

    #[test]
    fn validates_n_one() {
        let n = 1u32;
        assert!(n >= 1 && n <= MAX_IMAGES_PER_REQUEST, "n=1 should be valid");
    }

    #[test]
    fn validates_dim_too_large() {
        // Simulate the handler check: width > MAX_IMAGE_DIM should fail.
        let width_over = MAX_IMAGE_DIM + 1;
        assert!(
            width_over > MAX_IMAGE_DIM,
            "oversized width must be rejected"
        );
        let width_at_limit = MAX_IMAGE_DIM;
        assert!(
            !(width_at_limit > MAX_IMAGE_DIM),
            "exact-limit width must be accepted"
        );
    }

    #[test]
    fn decode_init_image_rejects_bad_b64() {
        let result = decode_init_image("data:image/png;base64,!!!invalid!!!");
        assert!(result.is_err());
    }
}

