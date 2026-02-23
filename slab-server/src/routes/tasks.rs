//! Generic task management endpoints.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::db::{TaskRecord, TaskStore};
use crate::error::ServerError;
use crate::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/tasks",               get(list_tasks))
        .route("/tasks/{id}",          get(get_task))
        .route("/tasks/{id}/result",   get(get_task_result))
        .route("/tasks/{id}/cancel",   post(cancel_task))
        .route("/tasks/{id}/restart",  post(restart_task))
}

#[derive(Deserialize)]
pub struct TaskTypeQuery {
    #[serde(rename = "type")]
    pub task_type: Option<String>,
}

#[derive(Serialize)]
pub struct TaskResponse {
    pub id: String,
    pub task_type: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

fn to_response(r: TaskRecord) -> TaskResponse {
    TaskResponse {
        id: r.id,
        task_type: r.task_type,
        status: r.status,
        created_at: r.created_at.to_rfc3339(),
        updated_at: r.updated_at.to_rfc3339(),
    }
}

pub async fn list_tasks(
    State(state): State<Arc<AppState>>,
    Query(q): Query<TaskTypeQuery>,
) -> Result<Json<Vec<TaskResponse>>, ServerError> {
    let records = state.store.list_tasks(q.task_type.as_deref()).await?;
    Ok(Json(records.into_iter().map(to_response).collect()))
}

pub async fn get_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<TaskResponse>, ServerError> {
    let record = state
        .store
        .get_task(&id)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;
    Ok(Json(to_response(record)))
}

pub async fn get_task_result(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let record = state
        .store
        .get_task(&id)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

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

    state.task_manager.cancel(&id);
    state
        .store
        .update_task_status(&id, "cancelled", None, None)
        .await?;
    info!(task_id = %id, "task cancelled");
    Ok(Json(serde_json::json!({ "status": "cancelled" })))
}

pub async fn restart_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let record = state
        .store
        .get_task(&id)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("task {id} not found")))?;

    if !matches!(record.status.as_str(), "failed" | "cancelled" | "interrupted") {
        return Err(ServerError::BadRequest(format!(
            "task {id} cannot be restarted (status: {})",
            record.status
        )));
    }

    // Reset to pending so the task can be picked up.
    state
        .store
        .update_task_status(&id, "pending", None, None)
        .await?;
    info!(task_id = %id, task_type = %record.task_type, "task reset to pending for restart");
    Ok(Json(serde_json::json!({ "task_id": id, "status": "pending" })))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn cancellable_statuses() {
        assert!(matches!("pending", "pending" | "running"));
        assert!(matches!("running", "pending" | "running"));
        assert!(!matches!("succeeded", "pending" | "running"));
    }

    #[test]
    fn restartable_statuses() {
        assert!(matches!("failed", "failed" | "cancelled" | "interrupted"));
        assert!(matches!("cancelled", "failed" | "cancelled" | "interrupted"));
        assert!(matches!("interrupted", "failed" | "cancelled" | "interrupted"));
        assert!(!matches!("running", "failed" | "cancelled" | "interrupted"));
    }
}
