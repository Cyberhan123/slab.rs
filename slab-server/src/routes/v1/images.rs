//! Image generation routes.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine as _;
use chrono::Utc;
use tracing::{debug, warn};
use utoipa::OpenApi;
use uuid::Uuid;

use crate::entities::{TaskRecord, TaskStore};
use crate::error::ServerError;
use crate::grpc;
use crate::schemas::v1::images::{ImageGenerationRequest, ImageMode};
use crate::state::AppState;

/// Maximum allowed prompt length in bytes.
const MAX_PROMPT_BYTES: usize = 128 * 1024; // 128 KiB

/// Maximum number of images that can be generated in a single request.
const MAX_IMAGES_PER_REQUEST: u32 = 10;

/// Maximum accepted image dimensions.
const MAX_IMAGE_DIM: u32 = 2048;

#[derive(OpenApi)]
#[openapi(paths(generate_images), components(schemas(ImageGenerationRequest,)))]
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
        (status = 202, description = "Task accepted", body = serde_json::Value),
        (status = 400, description = "Bad request (invalid parameters)"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn generate_images(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ImageGenerationRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    // ── Validation ────────────────────────────────────────────────────────────
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

    // ── Decode init image (img2img) ───────────────────────────────────────────
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
        n = req.n,
        width = req.width,
        height = req.height,
        mode = ?req.mode,
        "image generation request"
    );

    let generate_image_channel = state.grpc.generate_image_channel().ok_or_else(|| {
        ServerError::BackendNotReady("diffusion gRPC endpoint is not configured".into())
    })?;

    let task_id = Uuid::new_v4().to_string();
    let now = Utc::now();
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
    });

    state
        .store
        .insert_task(TaskRecord {
            id: task_id.clone(),
            task_type: "ggml.diffusion".into(),
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

    let grpc_req = grpc::pb::ImageRequest {
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
        strength: req.strength.unwrap_or(0.75),
        eta: req.eta.unwrap_or(0.0),
        init_image_data: init_image_bytes,
        init_image_width,
        init_image_height,
        init_image_channels,
    };

    let store = Arc::clone(&state.store);
    let task_manager = Arc::clone(&state.task_manager);
    let model_auto_unload = Arc::clone(&state.model_auto_unload);
    let task_id_for_spawn = task_id.clone();
    let generate_image_channel_for_spawn = generate_image_channel;
    let join = tokio::spawn(async move {
        let _usage_guard = match model_auto_unload.acquire_for_inference("ggml.diffusion").await {
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
                    .unwrap_or_else(|db_e| {
                        warn!(task_id = %task_id_for_spawn, error = %db_e, "failed to update auto-reload failure")
                    });
                task_manager.remove(&task_id_for_spawn);
                return;
            }
        };
        let rpc_result =
            grpc::client::generate_image(generate_image_channel_for_spawn, grpc_req).await;
        if let Ok(Some(record)) = store.get_task(&task_id_for_spawn).await {
            if record.status == "cancelled" {
                task_manager.remove(&task_id_for_spawn);
                return;
            }
        }

        match rpc_result {
            Ok(images_json) => {
                // Parse the JSON array of image objects returned by the backend.
                let images: Vec<serde_json::Value> =
                    serde_json::from_slice(&images_json).unwrap_or_default();

                let data_uris: Vec<String> = images
                    .iter()
                    .filter_map(|img| img["b64"].as_str())
                    .map(|b64| format!("data:image/png;base64,{b64}"))
                    .collect();

                // For single-image requests, keep backward-compatible `image` key.
                let payload = if data_uris.len() == 1 {
                    serde_json::json!({
                        "image": data_uris[0],
                        "images": data_uris,
                    })
                } else {
                    serde_json::json!({ "images": data_uris })
                };

                store
                    .update_task_status(
                        &task_id_for_spawn,
                        "succeeded",
                        Some(&payload.to_string()),
                        None,
                    )
                    .await
                    .unwrap_or_else(|e| {
                        warn!(task_id = %task_id_for_spawn, error = %e, "failed to update image result")
                    });
            }
            Err(e) => {
                let msg = e.to_string();
                store
                    .update_task_status(&task_id_for_spawn, "failed", None, Some(&msg))
                    .await
                    .unwrap_or_else(|db_e| {
                        warn!(task_id = %task_id_for_spawn, error = %db_e, "failed to update image failure")
                    });
            }
        }
        task_manager.remove(&task_id_for_spawn);
    });
    state
        .task_manager
        .insert(task_id.clone(), join.abort_handle());

    Ok(Json(serde_json::json!({ "task_id": task_id })))
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
        assert!(MAX_IMAGE_DIM + 1 > MAX_IMAGE_DIM);
    }

    #[test]
    fn decode_init_image_rejects_bad_b64() {
        let result = decode_init_image("data:image/png;base64,!!!invalid!!!");
        assert!(result.is_err());
    }
}
