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
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn generate_images(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<ImageGenerationRequest>,
) -> Result<Json<ImageGenerationResponse>, ServerError> {
    debug!(model = %req.model, prompt = %req.prompt, "image generation request");

    let result_bytes = slab_core::api::backend("ggml.diffusion")
        .op("inference_image")
        .input(slab_core::Payload::Json(serde_json::json!({
            "prompt": req.prompt,
            "n":      req.n,
            "size":   req.size.as_deref().unwrap_or("512x512"),
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
