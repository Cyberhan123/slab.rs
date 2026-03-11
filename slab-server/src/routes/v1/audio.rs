//! Audio transcription routes.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use tracing::{debug, warn};
use utoipa::OpenApi;
use uuid::Uuid;

use crate::entities::{TaskRecord, TaskStore};
use crate::error::ServerError;
use crate::grpc;
use crate::schemas::v1::audio::CompletionRequest;
use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(paths(transcribe))]
pub struct AudioApi;

/// Register audio routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/audio/transcriptions", post(transcribe))
}

/// Speech-to-text transcription (`POST /v1/audio/transcriptions`).
#[utoipa::path(
    post,
    path = "/v1/audio/transcriptions",
    tag = "audio",
    request_body(content = CompletionRequest, description = "Audio file path"),
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

    let transcribe_channel = state.grpc.transcribe_channel().ok_or_else(|| {
        ServerError::BackendNotReady("whisper gRPC endpoint is not configured".into())
    })?;

    let task_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    state
        .store
        .insert_task(TaskRecord {
            id: task_id.clone(),
            task_type: "ggml.whisper".into(),
            status: "running".into(),
            model_id: None,
            input_data: Some(req.path.clone()),
            result_data: None,
            error_msg: None,
            core_task_id: None,
            created_at: now,
            updated_at: now,
        })
        .await?;

    let grpc_req = grpc::pb::TranscribeRequest {
        path: req.path.clone(),
    };

    let store = Arc::clone(&state.store);
    let task_manager = Arc::clone(&state.task_manager);
    let model_auto_unload = Arc::clone(&state.model_auto_unload);
    let task_id_for_spawn = task_id.clone();
    let transcribe_channel_for_spawn = transcribe_channel;
    let join = tokio::spawn(async move {
        let _usage_guard = match model_auto_unload.acquire_for_inference("ggml.whisper").await {
            Ok(guard) => guard,
            Err(error) => {
                store
                    .update_task_status(
                        &task_id_for_spawn,
                        "failed",
                        None,
                        Some(&format!("whisper backend not ready: {error}")),
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
            grpc::client::transcribe(transcribe_channel_for_spawn, grpc_req.path).await;
        if let Ok(Some(record)) = store.get_task(&task_id_for_spawn).await {
            if record.status == "cancelled" {
                task_manager.remove(&task_id_for_spawn);
                return;
            }
        }

        match rpc_result {
            Ok(text) => {
                let payload = serde_json::json!({ "text": text }).to_string();
                store
                    .update_task_status(&task_id_for_spawn, "succeeded", Some(&payload), None)
                    .await
                    .unwrap_or_else(|e| {
                        warn!(task_id = %task_id_for_spawn, error = %e, "failed to update remote transcription result")
                    });
            }
            Err(e) => {
                let msg = e.to_string();
                store
                    .update_task_status(&task_id_for_spawn, "failed", None, Some(&msg))
                    .await
                    .unwrap_or_else(|db_e| {
                        warn!(task_id = %task_id_for_spawn, error = %db_e, "failed to update remote transcription failure")
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
mod test {}
