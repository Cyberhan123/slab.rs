//! Generic task management endpoints.
//!
//! Tasks backed by slab-core (whisper, image) have a `core_task_id` and use
//! `slab_core::api::status/result/cancel` for live status queries.
//! Server-only tasks (ffmpeg conversions, downloads) track status purely in DB.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};

use tracing::{info, warn};
use utoipa::OpenApi;

use crate::entities::TaskStore;
use crate::error::ServerError;
use crate::schemas::v1::task::TaskStatusEnumExt;
use crate::schemas::v1::task::{TaskResponse, TaskTypeQuery};
use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(
    paths(list_tasks, get_task, get_task_result, cancel_task, restart_task),
    components(schemas(TaskResponse, TaskTypeQuery))
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
    post,
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
    post,
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
            // Sync DB if status changed.
            if live_status != record.status {
                state
                    .store
                    .update_task_status(&id, live_status, None, None)
                    .await
                    .unwrap_or_else(|e| warn!(error = %e, "failed to sync task status"));
                record.status = live_status.to_owned();
            }
        }
    }

    Ok(Json(record.to_response()))
}

#[utoipa::path(
    post,
    path = "/v1/tasks/{id}/result",
    tag = "tasks",
    params(
        ("id" = String, Path, description = "ID of the task to retrieve result for")
    ),
    responses(
        (status = 200, description = "Task result retrieved", body = TaskResponse),
        (status = 400, description = "Bad request"),
        (status = 404, description = "Task not found"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn get_task_result(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let record = state
        .store
        .get_task(&id)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

    // For slab-core-backed tasks, fetch result from the runtime.
    if let Some(core_tid) = record.core_task_id {
        match slab_core::api::result(core_tid as u64).await {
            Ok(Some(payload)) => {
                let result_json = match &payload {
                    slab_core::Payload::Bytes(b) => {
                        let text = String::from_utf8_lossy(b).to_string();
                        serde_json::json!({ "text": text })
                    }
                    slab_core::Payload::Text(t) => serde_json::json!({ "text": t.to_string() }),
                    slab_core::Payload::Json(v) => v.clone(),
                    _ => serde_json::Value::Null,
                };
                // Persist result in DB for future queries.
                state
                    .store
                    .update_task_status(&id, "succeeded", Some(&result_json.to_string()), None)
                    .await
                    .unwrap_or_else(|e| warn!(error = %e, "failed to persist result"));
                return Ok(Json(result_json));
            }
            Ok(None) => {
                return Err(ServerError::BadRequest(format!(
                    "task {id} is not completed yet"
                )));
            }
            Err(e) => {
                return Err(ServerError::Runtime(e));
            }
        }
    }

    // Server-only tasks: read from DB.
    match record.status.as_str() {
        "succeeded" => {
            let result = record
                .result_data
                .map(|s| serde_json::from_str(&s).unwrap_or(serde_json::Value::String(s)))
                .unwrap_or(serde_json::Value::Null);
            Ok(Json(result))
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
) -> Result<Json<serde_json::Value>, ServerError> {
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
    Ok(Json(serde_json::json!({ "status": "cancelled" })))
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
        (status = 500, description = "Backend error"),
    )
)]
pub async fn restart_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ServerError> {
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

    // Re-submit to slab-core for tasks that have stored input.
    if let Some(input_json) = &record.input_data {
        match record.task_type.as_str() {
            "whisper" => {
                let input: serde_json::Value = match serde_json::from_str(input_json) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!(task_id = %id, error = %e, "invalid stored input_data for whisper restart");
                        return Err(ServerError::Internal(format!(
                            "invalid stored input_data: {e}"
                        )));
                    }
                };
                if let Some(tmp_path) = input["tmp_path"].as_str().map(str::to_owned) {
                    let task_id = id.clone();
                    let store = Arc::clone(&state.store);
                    state
                        .store
                        .update_task_status(&id, "running", None, None)
                        .await?;
                    tokio::spawn(async move {
                        // The preprocess stage provides the real PCM data; the initial
                        // Bytes payload is an empty placeholder that gets replaced by it.
                        let core_result = slab_core::api::backend(slab_core::api::Backend::GGMLWhisper)
                            .op(slab_core::api::Event::InferenceImage)
                            .input(slab_core::Payload::Bytes(std::sync::Arc::from(
                                [] as [u8; 0]
                            )))
                            .preprocess("ffmpeg.to_pcm_f32le", move |_| {
                                crate::routes::v1::audio::convert_to_pcm_f32le(&tmp_path)
                            })
                            .run()
                            .await;
                        match core_result {
                            Ok(core_task_id) => {
                                store
                                    .set_core_task_id(&task_id, core_task_id as i64)
                                    .await
                                    .ok();
                            }
                            Err(e) => {
                                store
                                    .update_task_status(
                                        &task_id,
                                        "failed",
                                        None,
                                        Some(&e.to_string()),
                                    )
                                    .await
                                    .ok();
                            }
                        }
                    });
                }
            }
            "image" => {
                let input: serde_json::Value = match serde_json::from_str(input_json) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!(task_id = %id, error = %e, "invalid stored input_data for image restart");
                        return Err(ServerError::Internal(format!(
                            "invalid stored input_data: {e}"
                        )));
                    }
                };
                let task_id = id.clone();
                let store = Arc::clone(&state.store);
                state
                    .store
                    .update_task_status(&id, "running", None, None)
                    .await?;
                tokio::spawn(async move {
                    let core_result = slab_core::api::backend(slab_core::api::Backend::GGMLDiffusion)
                        .op(slab_core::api::Event::InferenceImage)
                        .input(slab_core::Payload::Json(input))
                        .run()
                        .await;
                    match core_result {
                        Ok(core_task_id) => {
                            store
                                .set_core_task_id(&task_id, core_task_id as i64)
                                .await
                                .ok();
                        }
                        Err(e) => {
                            store
                                .update_task_status(&task_id, "failed", None, Some(&e.to_string()))
                                .await
                                .ok();
                        }
                    }
                });
            }
            _ => {
                // For server-only tasks (ffmpeg, downloads), reset to pending for manual
                // operator handling.  Future iterations could re-spawn these too.
                state
                    .store
                    .update_task_status(&id, "pending", None, None)
                    .await?;
                info!(task_id = %id, task_type = %record.task_type, "task reset to pending for restart");
                return Ok(Json(
                    serde_json::json!({ "task_id": id, "status": "pending" }),
                ));
            }
        }
    }

    info!(task_id = %id, task_type = %record.task_type, "task restarted");
    Ok(Json(
        serde_json::json!({ "task_id": id, "status": "running" }),
    ))
}
