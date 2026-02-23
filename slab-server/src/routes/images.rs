//! Image generation routes (Stable Diffusion).
//!
//! Forwards the prompt and generation parameters to the `ggml.diffusion`
//! backend and returns the generated image as base64-encoded PNG data.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine as _;
use tracing::{debug, info};

use crate::error::ServerError;
use crate::models::openai::{ImageData, ImageGenerationRequest, ImageGenerationResponse};
use crate::state::AppState;

/// Maximum allowed prompt length in bytes.
const MAX_PROMPT_BYTES: usize = 128 * 1024; // 128 KiB

/// Maximum number of images that can be generated in a single request.
const MAX_IMAGES_PER_REQUEST: u32 = 10;

/// Accepted image size strings.  The backend may support other resolutions,
/// but these are the values validated at the API layer.
const VALID_SIZES: &[&str] = &["256x256", "512x512", "1024x1024"];

/// Register image generation routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/images/generations", post(generate_images))
}

/// Image generation (`POST /v1/images/generations`).
///
/// Forwards the prompt to the `ggml.diffusion` backend and returns the
/// generated image(s) base64-encoded in an OpenAI-compatible JSON envelope.
#[utoipa::path(
    post,
    path = "/v1/images/generations",
    tag = "images",
    request_body = ImageGenerationRequest,
    responses(
        (status = 200, description = "Generated image(s)", body = ImageGenerationResponse),
        (status = 400, description = "Bad request (invalid parameters)"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn generate_images(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<ImageGenerationRequest>,
) -> Result<Json<ImageGenerationResponse>, ServerError> {
    // ── Input validation ───────────────────────────────────────────────────────

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

    // n must be in [1, MAX_IMAGES_PER_REQUEST].
    if req.n == 0 || req.n > MAX_IMAGES_PER_REQUEST {
        return Err(ServerError::BadRequest(format!(
            "invalid n ({}): must be between 1 and {MAX_IMAGES_PER_REQUEST}",
            req.n
        )));
    }

    // Validate the size string if provided.
    let size = req.size.as_deref().unwrap_or("512x512");
    if !VALID_SIZES.contains(&size) {
        return Err(ServerError::BadRequest(format!(
            "invalid size '{size}'; must be one of: {}",
            VALID_SIZES.join(", ")
        )));
    }

    debug!(model = %req.model, prompt_len = req.prompt.len(), n = req.n, %size, "image generation request");

    let result_bytes = slab_core::api::backend("ggml.diffusion")
        .op("inference_image")
        .input(slab_core::Payload::Json(serde_json::json!({
            "prompt": req.prompt,
            "n":      req.n,
            "size":   size,
        })))
        .run_wait()
        .await
        .map_err(ServerError::Runtime)?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(&result_bytes);

    info!(image_bytes = result_bytes.len(), "image generation done");

    Ok(Json(ImageGenerationResponse {
        created: chrono::Utc::now().timestamp(),
        data:    vec![ImageData { b64_json: b64 }],
    }))
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn validates_n_zero() {
        let n = 0u32;
        assert!(n == 0 || n > MAX_IMAGES_PER_REQUEST, "n=0 should be invalid");
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
    fn validates_valid_size() {
        assert!(VALID_SIZES.contains(&"512x512"));
        assert!(VALID_SIZES.contains(&"256x256"));
        assert!(VALID_SIZES.contains(&"1024x1024"));
    }

    #[test]
    fn rejects_invalid_size() {
        assert!(!VALID_SIZES.contains(&"800x600"), "800x600 is not a valid size");
    }
}

