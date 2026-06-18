use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::path::IdPath;
use crate::api::v1::tasks::schema::{
    TaskProgressResponse, TaskResponse, TaskResultPayload, TaskStatus, TaskTypeQuery,
    TimedTextSegmentResponse,
};
use crate::api::validation::{ValidatedQuery, validate};
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::TaskApplicationService;

#[derive(OpenApi)]
#[openapi(
    paths(list_tasks, get_task, get_task_result, cancel_task, restart_task),
    components(schemas(
        TaskProgressResponse,
        TaskResponse,
        TaskResultPayload,
        TimedTextSegmentResponse,
        TaskStatus,
        TaskTypeQuery
    ))
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
async fn list_tasks(
    State(service): State<TaskApplicationService>,
    ValidatedQuery(q): ValidatedQuery<TaskTypeQuery>,
) -> Result<Json<Vec<TaskResponse>>, ServerError> {
    let tasks =
        service.list_tasks(q.task_type.as_deref()).await?.into_iter().map(Into::into).collect();
    Ok(Json(tasks))
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
async fn get_task(
    State(service): State<TaskApplicationService>,
    Path(params): Path<IdPath>,
) -> Result<Json<TaskResponse>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.get_task(&params.id).await?.into()))
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
async fn get_task_result(
    State(service): State<TaskApplicationService>,
    Path(params): Path<IdPath>,
) -> Result<Json<TaskResultPayload>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.get_task_result(&params.id).await?.into()))
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
async fn cancel_task(
    State(service): State<TaskApplicationService>,
    Path(params): Path<IdPath>,
) -> Result<Json<TaskResponse>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.cancel_task(&params.id).await?.into()))
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
        (status = 409, description = "Task restart conflicts with active work"),
        (status = 404, description = "Task not found"),
        (status = 500, description = "Backend error"),
    )
)]
async fn restart_task(
    State(service): State<TaskApplicationService>,
    Path(params): Path<IdPath>,
) -> Result<Json<TaskResponse>, ServerError> {
    let params = validate(params)?;
    Ok(Json(service.restart_task(&params.id).await?.into()))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use chrono::Utc;
    use slab_app_core::domain::models::TaskStatus as DomainTaskStatus;
    use slab_app_core::infra::db::{TaskRecord, TaskStore};

    use crate::api::test_support::TestServer;

    #[tokio::test]
    async fn list_tasks_rejects_blank_type_query() {
        let server = TestServer::new().await;

        let response = server.get("/v1/tasks?type=%20").await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        assert!(response.body["message"].as_str().unwrap_or_default().contains("type"));
    }

    #[tokio::test]
    async fn get_missing_task_maps_to_not_found() {
        let server = TestServer::new().await;

        let response = server.get("/v1/tasks/missing-task").await;

        assert_eq!(response.status, StatusCode::NOT_FOUND);
        assert_eq!(response.body["i18n"]["message"]["key"], "server.errors.notFound");
    }

    #[tokio::test]
    async fn get_task_result_requires_succeeded_status() {
        let server = TestServer::new().await;
        server
            .store
            .insert_task(task_record("task-running", "image_generation", DomainTaskStatus::Running))
            .await
            .expect("seed task");

        let response = server.get("/v1/tasks/task-running/result").await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        assert!(
            response.body["message"].as_str().unwrap_or_default().contains("task is not succeeded")
        );
    }

    #[tokio::test]
    async fn cancel_task_updates_active_task_status() {
        let server = TestServer::new().await;
        server
            .store
            .insert_task(task_record("task-cancel", "image_generation", DomainTaskStatus::Running))
            .await
            .expect("seed task");

        let response =
            server.post_json("/v1/tasks/task-cancel/cancel", serde_json::Value::Null).await;

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.body["id"], "task-cancel");
        assert_eq!(response.body["status"], "cancelled");
    }

    #[tokio::test]
    async fn restart_rejects_non_model_download_tasks() {
        let server = TestServer::new().await;
        server
            .store
            .insert_task(task_record("task-restart", "image_generation", DomainTaskStatus::Failed))
            .await
            .expect("seed task");

        let response =
            server.post_json("/v1/tasks/task-restart/restart", serde_json::Value::Null).await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        assert!(
            response.body["message"]
                .as_str()
                .unwrap_or_default()
                .contains("does not support restart")
        );
    }

    fn task_record(id: &str, task_type: &str, status: DomainTaskStatus) -> TaskRecord {
        let now = Utc::now();
        TaskRecord {
            id: id.to_owned(),
            task_type: task_type.to_owned(),
            status,
            model_id: None,
            input_data: None,
            result_data: Some(r#"{"text":"done"}"#.to_owned()),
            error_msg: if status == DomainTaskStatus::Failed {
                Some("failed".to_owned())
            } else {
                None
            },
            core_task_id: None,
            created_at: now,
            updated_at: now,
        }
    }
}
