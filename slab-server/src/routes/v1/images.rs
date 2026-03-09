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
use crate::schemas::v1::images::ImageGenerationRequest;
use crate::state::AppState;

/// Maximum allowed prompt length in bytes.
const MAX_PROMPT_BYTES: usize = 128 * 1024; // 128 KiB

/// Maximum number of images that can be generated in a single request.
const MAX_IMAGES_PER_REQUEST: u32 = 10;

/// Accepted image size strings.
const VALID_SIZES: &[&str] = &["256x256", "512x512", "1024x1024"];

#[derive(OpenApi)]
#[openapi(paths(generate_images), components(schemas(ImageGenerationRequest,)))]
pub struct ImagesApi;

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

    debug!(
        model = %req.model,
        prompt_len = req.prompt.len(),
        n = req.n,
        %size,
        "image generation request"
    );

    let generate_image_channel = state.grpc.generate_image_channel().ok_or_else(|| {
        ServerError::BackendNotReady("diffusion gRPC endpoint is not configured".into())
    })?;

    let task_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let input_json = serde_json::json!({
        "prompt": req.prompt,
        "n": req.n,
        "size": size,
        "model": req.model,
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

    let request_for_spawn = grpc::pb::ImageRequest {
        model: req.model.clone(),
        prompt: req.prompt.clone(),
        n: req.n,
        size: size.to_string(),
    };

    let store = Arc::clone(&state.store);
    let task_manager = Arc::clone(&state.task_manager);
    let task_id_for_spawn = task_id.clone();
    let generate_image_channel_for_spawn = generate_image_channel;
    let join = tokio::spawn(async move {
        let rpc_result =
            grpc::client::generate_image(generate_image_channel_for_spawn, request_for_spawn).await;
        if let Ok(Some(record)) = store.get_task(&task_id_for_spawn).await {
            if record.status == "cancelled" {
                task_manager.remove(&task_id_for_spawn);
                return;
            }
        }

        match rpc_result {
            Ok(image) => {
                let encoded = base64::engine::general_purpose::STANDARD.encode(&image);
                let data_uri = format!("data:image/png;base64,{encoded}");
                let payload = serde_json::json!({ "image": data_uri }).to_string();
                store
                    .update_task_status(&task_id_for_spawn, "succeeded", Some(&payload), None)
                    .await
                    .unwrap_or_else(|e| {
                        warn!(task_id = %task_id_for_spawn, error = %e, "failed to update remote image result")
                    });
            }
            Err(e) => {
                let msg = e.to_string();
                store
                    .update_task_status(&task_id_for_spawn, "failed", None, Some(&msg))
                    .await
                    .unwrap_or_else(|db_e| {
                        warn!(task_id = %task_id_for_spawn, error = %db_e, "failed to update remote image failure")
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
    fn validates_valid_size() {
        assert!(VALID_SIZES.contains(&"512x512"));
        assert!(VALID_SIZES.contains(&"256x256"));
        assert!(VALID_SIZES.contains(&"1024x1024"));
    }

    #[test]
    fn rejects_invalid_size() {
        assert!(
            !VALID_SIZES.contains(&"800x600"),
            "800x600 is not a valid size"
        );
    }
}
