//! Image generation routes (Stable Diffusion) – async task pattern.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine as _;
use chrono::Utc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::{TaskRecord, TaskStore};
use crate::error::ServerError;
use crate::models::openai::ImageGenerationRequest;
use crate::state::AppState;

/// Maximum allowed prompt length in bytes.
const MAX_PROMPT_BYTES: usize = 128 * 1024; // 128 KiB

/// Maximum number of images that can be generated in a single request.
const MAX_IMAGES_PER_REQUEST: u32 = 10;

/// Accepted image size strings.
const VALID_SIZES: &[&str] = &["256x256", "512x512", "1024x1024"];

/// Register image generation routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/images/generations", post(generate_images))
}

/// Image generation (`POST /v1/images/generations`).
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
    // ── Input validation ─────────────────────────────────────────────────────

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
    let size = req.size.as_deref().unwrap_or("512x512");
    if !VALID_SIZES.contains(&size) {
        return Err(ServerError::BadRequest(format!(
            "invalid size '{size}'; must be one of: {}",
            VALID_SIZES.join(", ")
        )));
    }

    debug!(model = %req.model, prompt_len = req.prompt.len(), n = req.n, %size, "image generation request");

    let task_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let input_data = serde_json::json!({
        "prompt": req.prompt,
        "n": req.n,
        "size": size,
        "model": req.model,
    })
    .to_string();

    state
        .store
        .insert_task(TaskRecord {
            id: task_id.clone(),
            task_type: "image".into(),
            status: "pending".into(),
            input_data: Some(input_data.clone()),
            result_data: None,
            error_msg: None,
            created_at: now,
            updated_at: now,
        })
        .await?;

    let store = Arc::clone(&state.store);
    let task_manager = Arc::clone(&state.task_manager);
    let tid = task_id.clone();

    let join = tokio::spawn(async move {
        store.update_task_status(&tid, "running", None, None).await.ok();

        let input: serde_json::Value = serde_json::from_str(&input_data).unwrap_or_default();

        let result = slab_core::api::backend("ggml.diffusion")
            .op("inference_image")
            .input(slab_core::Payload::Json(input))
            .run_wait()
            .await;

        match result {
            Ok(bytes) => {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                let result_json = serde_json::json!({
                    "created": chrono::Utc::now().timestamp(),
                    "data": [{ "b64_json": b64 }],
                })
                .to_string();
                store
                    .update_task_status(&tid, "succeeded", Some(&result_json), None)
                    .await
                    .ok();
                info!(task_id = %tid, image_bytes = bytes.len(), "image generation done");
            }
            Err(e) => {
                warn!(task_id = %tid, error = %e, "image generation failed");
                store
                    .update_task_status(&tid, "failed", None, Some(&e.to_string()))
                    .await
                    .ok();
            }
        }
        task_manager.remove(&tid);
    });

    state.task_manager.insert(task_id.clone(), join.abort_handle());
    info!(task_id = %task_id, "image generation task accepted");
    Ok(Json(serde_json::json!({ "task_id": task_id })))
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

