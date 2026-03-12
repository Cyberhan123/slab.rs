//! Generic task management endpoints.
//!
//! Tasks backed by slab-core (whisper, image) have a `core_task_id` and use
//! `slab_core::api::status/result/cancel` for live status queries.
//! Server-only tasks (ffmpeg conversions, downloads) track status purely in DB.

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
                            let uri = format!("data:image/png;base64,{encoded}");
                            TaskResultPayload {
                                image: Some(uri.clone()),
                                images: Some(vec![uri]),
                                video_path: None,
                                text: None,
                            }
                        } else {
                            TaskResultPayload {
                                image: None,
                                images: None,
                                video_path: None,
                                text: Some(String::from_utf8_lossy(b).to_string()),
                            }
                        }
                    }
                    slab_core::Payload::Text(t) => TaskResultPayload {
                        image: None,
                        images: None,
                        video_path: None,
                        text: Some(t.to_string()),
                    },
                    slab_core::Payload::Json(v) => {
                        // `Payload::Json` is rare for inference results (it is mainly used
                        // for control payloads such as model-load parameters).  When it
                        // does appear we try to map well-known keys:
                        //   • "image" / "images" → TaskResultPayload.image / .images
                        //   • "video_path"        → TaskResultPayload.video_path
                        //   • "text"              → TaskResultPayload.text
                        // If none are present the entire JSON value is serialised to
                        // a compact string and stored in `text` so callers can still
                        // inspect the raw payload rather than receiving an empty response.
                        let image =
                            v.get("image").and_then(|s| s.as_str()).map(str::to_owned);
                        let images = v.get("images").and_then(|a| {
                            a.as_array().map(|arr| {
                                arr.iter()
                                    .filter_map(|s| s.as_str().map(str::to_owned))
                                    .collect::<Vec<_>>()
                            })
                        });
                        let video_path =
                            v.get("video_path").and_then(|s| s.as_str()).map(str::to_owned);
                        let text = v
                            .get("text")
                            .and_then(|s| s.as_str())
                            .map(str::to_owned)
                            .or_else(|| {
                                if image.is_none() && video_path.is_none() {
                                    Some(v.to_string())
                                } else {
                                    None
                                }
                            });
                        TaskResultPayload { image, images, video_path, text }
                    }
                    _ => TaskResultPayload {
                        image: None,
                        images: None,
                        video_path: None,
                        text: None,
                    },
                };
                // Persist result in DB for future queries.
                match serde_json::to_string(&result_payload) {
                    Ok(result_json) => {
                        state
                            .store
                            .update_task_status(&id, "succeeded", Some(&result_json), None)
                            .await
                            .unwrap_or_else(|e| warn!(error = %e, "failed to persist result"));
                    }
                    Err(e) => {
                        warn!(task_id = %id, error = %e, "failed to serialize result_payload; result will not be persisted to DB");
                    }
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
                                images: None,
                                video_path: None,
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
                            images: None,
                            video_path: None,
                            text: Some(data),
                        }
                    })
                })
                .unwrap_or(TaskResultPayload {
                    image: None,
                    images: None,
                    video_path: None,
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

    use crate::schemas::v1::task::TaskResultPayload;

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Replicates the `Payload::Bytes` → `TaskResultPayload` mapping from
    /// `get_task_result` so we can exercise the conversion logic without
    /// standing up the full HTTP stack.
    fn bytes_to_result_payload(task_type: &str, bytes: &[u8]) -> TaskResultPayload {
        if task_type == "image" {
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            let uri = format!("data:image/png;base64,{encoded}");
            TaskResultPayload {
                image: Some(uri.clone()),
                images: Some(vec![uri]),
                video_path: None,
                text: None,
            }
        } else {
            TaskResultPayload {
                image: None,
                images: None,
                video_path: None,
                text: Some(String::from_utf8_lossy(bytes).to_string()),
            }
        }
    }

    // ── Payload::Bytes (image) ────────────────────────────────────────────────

    #[test]
    fn image_bytes_become_data_uri() {
        let png_bytes = b"\x89PNG\r\n\x1a\nfakedata";
        let result = bytes_to_result_payload("image", png_bytes);

        assert!(result.text.is_none(), "text field must be absent for image tasks");

        let image_field = result.image.expect("image field must be present");
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
    fn image_task_has_no_text_field() {
        let result = bytes_to_result_payload("image", b"\x00\x01\x02");
        assert!(result.text.is_none(), "text field must be absent for image tasks");
        assert!(result.image.is_some(), "image field must be present for image tasks");
    }

    // ── Payload::Bytes (non-image) ────────────────────────────────────────────

    #[test]
    fn non_image_bytes_become_text() {
        let result = bytes_to_result_payload("whisper", b"hello transcription");
        assert_eq!(result.text.as_deref(), Some("hello transcription"));
        assert!(result.image.is_none(), "image field must be absent for non-image tasks");
    }

    // ── Payload::Json mapping ────────────────────────────────────────────────

    /// Helper that replicates the `Payload::Json` → `TaskResultPayload` mapping.
    fn json_to_result_payload(v: &serde_json::Value) -> TaskResultPayload {
        let image = v.get("image").and_then(|s| s.as_str()).map(str::to_owned);
        let images = v.get("images").and_then(|a| {
            a.as_array().map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            })
        });
        let video_path = v.get("video_path").and_then(|s| s.as_str()).map(str::to_owned);
        let text = v
            .get("text")
            .and_then(|s| s.as_str())
            .map(str::to_owned)
            .or_else(|| {
                if image.is_none() && video_path.is_none() {
                    Some(v.to_string())
                } else {
                    None
                }
            });
        TaskResultPayload { image, images, video_path, text }
    }

    #[test]
    fn json_payload_with_text_key() {
        let v = serde_json::json!({ "text": "hello from json" });
        let result = json_to_result_payload(&v);
        assert_eq!(result.text.as_deref(), Some("hello from json"));
        assert!(result.image.is_none());
    }

    #[test]
    fn json_payload_with_image_key() {
        let v = serde_json::json!({ "image": "data:image/png;base64,abc=" });
        let result = json_to_result_payload(&v);
        assert_eq!(result.image.as_deref(), Some("data:image/png;base64,abc="));
        assert!(result.text.is_none());
    }

    #[test]
    fn json_payload_unknown_keys_fall_back_to_text() {
        let v = serde_json::json!({ "some_key": 42 });
        let result = json_to_result_payload(&v);
        // Unknown JSON is serialised as a compact string into the `text` field.
        assert!(result.image.is_none());
        assert!(
            result.text.as_deref().unwrap_or("").contains("some_key"),
            "full JSON should be in text when no known keys present"
        );
    }

    // ── DB round-trip ────────────────────────────────────────────────────────

    #[test]
    fn task_result_payload_roundtrips_through_json() {
        let original = TaskResultPayload {
            image: Some("data:image/png;base64,abc=".to_owned()),
            images: Some(vec!["data:image/png;base64,abc=".to_owned()]),
            video_path: None,
            text: None,
        };
        let serialized = serde_json::to_string(&original).expect("serialize");
        let deserialized: TaskResultPayload =
            serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(deserialized.image, original.image);
        assert_eq!(deserialized.images, original.images);
        assert_eq!(deserialized.text, original.text);
    }

    #[test]
    fn task_result_payload_text_roundtrips_through_json() {
        let original = TaskResultPayload {
            image: None,
            images: None,
            video_path: None,
            text: Some("hello world".to_owned()),
        };
        let serialized = serde_json::to_string(&original).expect("serialize");
        let deserialized: TaskResultPayload =
            serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(deserialized.image, original.image);
        assert_eq!(deserialized.text, original.text);
    }
}
