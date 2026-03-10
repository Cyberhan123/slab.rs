//! Generic task management endpoints.
//!
//! Tasks backed by slab-core (whisper, image) have a `core_task_id` and use
//! `slab_core::api::status/result/cancel` for live status queries.
//! Server-only tasks (ffmpeg conversions, downloads) track status purely in DB.

use std::str::FromStr;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine as _;

use tracing::{info, warn};
use utoipa::OpenApi;

use crate::entities::TaskStore;
use crate::error::ServerError;
use crate::schemas::v1::task::TaskStatusEnumExt;
use crate::schemas::v1::task::{TaskResponse, TaskResultPayload, TaskTypeQuery};
use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(
    paths(list_tasks, get_task, get_task_result, cancel_task, restart_task),
    components(schemas(TaskResponse, TaskResultPayload, TaskTypeQuery))
)]
pub struct TasksApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/tasks", get(list_tasks))
        .route("/tasks/{id}", get(get_task))
        .route("/tasks/{id}/result", get(get_task_result))
        .route("/tasks/{id}/cancel", post(cancel_task))
        .route("/tasks/{id}/restart", post(restart_task))
}

#[utoipa::path(
    get,
    path = "/v1/tasks",
    tag = "tasks",
        params(TaskTypeQuery),
    responses(
        (status = 200, description = "Tasks listed", body = [TaskResponse]),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn list_tasks(
    State(state): State<Arc<AppState>>,
    Query(q): Query<TaskTypeQuery>,
) -> Result<Json<Vec<TaskResponse>>, ServerError> {
    let records = state.store.list_tasks(q.task_type.as_deref()).await?;
    Ok(Json(records.into_iter().map(|r| r.to_response()).collect()))
}

#[utoipa::path(
    get,
    path = "/v1/tasks/{id}",
    tag = "tasks",
    params(
        ("id" = String, Path, description = "ID of the task to retrieve")
    ),
    responses(
        (status = 200, description = "Task retrieved", body = TaskResponse),
        (status = 400, description = "Bad request"),
        (status = 404, description = "Task not found"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn get_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<TaskResponse>, ServerError> {
    let mut record = state
        .store
        .get_task(&id)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

    // For slab-core-backed tasks, refresh status from the runtime.
    if let Some(core_tid) = record.core_task_id {
        if let Ok(view) = slab_core::api::status(core_tid as u64).await {
            let live_status = view.status.as_str();
            let live_error = match &view.status {
                slab_core::TaskStatus::Failed { error } => Some(error.to_string()),
                _ => None,
            };
            // Sync DB if status changed.
            if live_status != record.status || live_error.as_deref() != record.error_msg.as_deref()
            {
                state
                    .store
                    .update_task_status(&id, live_status, None, live_error.as_deref())
                    .await
                    .unwrap_or_else(|e| warn!(error = %e, "failed to sync task status"));
                record.status = live_status.to_owned();
                record.error_msg = live_error;
            }
        }
    }

    Ok(Json(record.to_response()))
}

#[utoipa::path(
    get,
    path = "/v1/tasks/{id}/result",
    tag = "tasks",
    params(
        ("id" = String, Path, description = "ID of the task to retrieve result for")
    ),
    responses(
        (status = 200, description = "Task result retrieved", body = TaskResultPayload),
        (status = 400, description = "Bad request"),
        (status = 404, description = "Task not found"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn get_task_result(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<TaskResultPayload>, ServerError> {
    let record = state
        .store
        .get_task(&id)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

    // For slab-core-backed tasks, fetch result from the runtime.
    if let Some(core_tid) = record.core_task_id {
        match slab_core::api::result(core_tid as u64).await {
            Ok(Some(payload)) => {
                let result_payload = match &payload {
                    slab_core::Payload::Bytes(b) => {
                        // Image tasks return raw PNG bytes; encode them as a data URI.
                        if record.task_type == "image" {
                            let encoded =
                                base64::engine::general_purpose::STANDARD.encode(b.as_ref());
                            TaskResultPayload {
                                image: Some(format!("data:image/png;base64,{encoded}")),
                                text: None,
                            }
                        } else {
                            TaskResultPayload {
                                image: None,
                                text: Some(String::from_utf8_lossy(b).to_string()),
                            }
                        }
                    }
                    slab_core::Payload::Text(t) => TaskResultPayload {
                        image: None,
                        text: Some(t.to_string()),
                    },
                    slab_core::Payload::Json(v) => {
                        // Extract known fields; fall back to serialized JSON string in `text`.
                        let image =
                            v.get("image").and_then(|s| s.as_str()).map(str::to_owned);
                        let text = v
                            .get("text")
                            .and_then(|s| s.as_str())
                            .map(str::to_owned)
                            .or_else(|| {
                                if image.is_none() {
                                    Some(v.to_string())
                                } else {
                                    None
                                }
                            });
                        TaskResultPayload { image, text }
                    }
                    _ => TaskResultPayload {
                        image: None,
                        text: None,
                    },
                };
                // Persist result in DB for future queries.
                if let Ok(result_json) = serde_json::to_string(&result_payload) {
                    state
                        .store
                        .update_task_status(&id, "succeeded", Some(&result_json), None)
                        .await
                        .unwrap_or_else(|e| warn!(error = %e, "failed to persist result"));
                }
                return Ok(Json(result_payload));
            }
            Ok(None) => {
                // `api::result()` returns None when the task is still in
                // progress *or* when the payload was already consumed
                // (ResultConsumed).  Fall back to the persisted result in DB
                // if it was written by a prior call.
                if let Some(data) = record.result_data {
                    let result_payload =
                        serde_json::from_str::<TaskResultPayload>(&data).unwrap_or_else(|e| {
                            warn!(task_id = %id, error = %e, "persisted result_data is not a TaskResultPayload; returning as text");
                            TaskResultPayload {
                                image: None,
                                text: Some(data),
                            }
                        });
                    return Ok(Json(result_payload));
                }
                return Err(ServerError::BadRequest(format!(
                    "task {id} is not completed yet"
                )));
            }
            Err(e) => {
                let err_msg = e.to_string();
                state
                    .store
                    .update_task_status(&id, "failed", None, Some(&err_msg))
                    .await
                    .unwrap_or_else(
                        |db_e| warn!(error = %db_e, "failed to sync failed task error"),
                    );
                return Err(ServerError::Runtime(e));
            }
        }
    }

    // Server-only tasks: read from DB.
    match record.status.as_str() {
        "succeeded" => {
            let result_payload = record
                .result_data
                .map(|data| {
                    serde_json::from_str::<TaskResultPayload>(&data).unwrap_or_else(|e| {
                        warn!(task_id = %id, error = %e, "persisted result_data is not a TaskResultPayload; returning as text");
                        TaskResultPayload {
                            image: None,
                            text: Some(data),
                        }
                    })
                })
                .unwrap_or(TaskResultPayload {
                    image: None,
                    text: None,
                });
            Ok(Json(result_payload))
        }
        status => Err(ServerError::BadRequest(format!(
            "task is not succeeded (status: {status})"
        ))),
    }
}

#[utoipa::path(
    post,
    path = "/v1/tasks/{id}/cancel",
    tag = "tasks",
    params(
        ("id" = String, Path, description = "ID of the task to cancel")
    ),
    responses(
        (status = 200, description = "Task cancelled", body = TaskResponse),
        (status = 400, description = "Bad request"),
        (status = 404, description = "Task not found"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn cancel_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<TaskResponse>, ServerError> {
    let record = state
        .store
        .get_task(&id)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

    if !matches!(record.status.as_str(), "pending" | "running") {
        return Err(ServerError::BadRequest(format!(
            "task {id} is not cancellable (status: {})",
            record.status
        )));
    }

    // Update DB status first to prevent a race where the task could write
    // "succeeded" or "failed" after we abort it.
    state
        .store
        .update_task_status(&id, "cancelled", None, None)
        .await?;

    // Cancel in slab-core runtime (if applicable).
    if let Some(core_tid) = record.core_task_id {
        if let Err(e) = slab_core::api::cancel(core_tid as u64) {
            warn!(task_id = %id, error = %e, "failed to cancel slab-core task");
        }
    }
    // Also abort any server-side tokio handle.
    state.task_manager.cancel(&id);

    info!(task_id = %id, "task cancelled");
    // Re-fetch the updated record so the response reflects the persisted state.
    let updated = state
        .store
        .get_task(&id)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("task {id} not found after cancel")))?;
    Ok(Json(updated.to_response()))
}

#[utoipa::path(
    post,
    path = "/v1/tasks/{id}/restart",
    tag = "tasks",
    params(
        ("id" = String, Path, description = "ID of the task to restart")
    ),
    responses(
        (status = 200, description = "Task restarted", body = TaskResponse),
        (status = 400, description = "Bad request"),
        (status = 404, description = "Task not found"),
        (status = 501, description = "Not implemented"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn restart_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<TaskResponse>, ServerError> {
    let record = state
        .store
        .get_task(&id)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

    if !matches!(
        record.status.as_str(),
        "failed" | "cancelled" | "interrupted"
    ) {
        return Err(ServerError::BadRequest(format!(
            "task {id} cannot be restarted (status: {})",
            record.status
        )));
    }

    Err(ServerError::NotImplemented(
        "task restart is not yet implemented".to_owned(),
    ))
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use base64::Engine as _;

    /// Helper that replicates the image-vs-text branching used in `get_task_result`.
    fn bytes_to_result_json(task_type: &str, bytes: &[u8]) -> serde_json::Value {
        if task_type == "image" {
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            let data_uri = format!("data:image/png;base64,{encoded}");
            serde_json::json!({ "image": data_uri })
        } else {
            let text = String::from_utf8_lossy(bytes).to_string();
            serde_json::json!({ "text": text })
        }
    }

    #[test]
    fn image_bytes_become_data_uri() {
        let png_bytes = b"\x89PNG\r\n\x1a\nfakedata";
        let result = bytes_to_result_json("image", png_bytes);
        let image_field = result["image"]
            .as_str()
            .expect("image field must be a string");
        assert!(
            image_field.starts_with("data:image/png;base64,"),
            "image field should start with PNG data URI prefix"
        );
        // Decode and verify round-trip.
        let b64_part = image_field.trim_start_matches("data:image/png;base64,");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(b64_part)
            .expect("base64 should decode cleanly");
        assert_eq!(decoded, png_bytes);
    }

    #[test]
    fn non_image_bytes_become_text() {
        let text_bytes = b"hello transcription";
        let result = bytes_to_result_json("whisper", text_bytes);
        assert_eq!(result["text"].as_str(), Some("hello transcription"));
        assert!(result.get("image").is_none(), "image field must be absent");
    }

    #[test]
    fn image_task_has_no_text_field() {
        let result = bytes_to_result_json("image", b"\x00\x01\x02");
        assert!(
            result.get("text").is_none(),
            "text field must be absent for image tasks"
        );
        assert!(result["image"].is_string());
    }
}
