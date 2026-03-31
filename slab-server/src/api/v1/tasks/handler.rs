use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use utoipa::OpenApi;
use validator::Validate;

use crate::api::v1::tasks::schema::{TaskResponse, TaskResultPayload, TaskStatus, TaskTypeQuery};
use crate::api::validation::{ValidatedQuery, validate};
use crate::context::AppState;
use crate::domain::services::TaskApplicationService;
use crate::error::ServerError;

#[derive(OpenApi)]
#[openapi(
    paths(list_tasks, get_task, get_task_result, cancel_task, restart_task),
    components(schemas(TaskResponse, TaskResultPayload, TaskStatus, TaskTypeQuery))
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
    Path(params): Path<TaskIdPath>,
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
    Path(params): Path<TaskIdPath>,
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
    Path(params): Path<TaskIdPath>,
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
        (status = 400, description = "Bad request"),
        (status = 404, description = "Task not found"),
        (status = 501, description = "Not implemented"),
        (status = 500, description = "Backend error"),
    )
)]
async fn restart_task(
    State(service): State<TaskApplicationService>,
    Path(params): Path<TaskIdPath>,
) -> Result<Json<TaskResponse>, ServerError> {
    let params = validate(params)?;
    service.validate_restartable(&params.id).await?;

    Err(ServerError::NotImplemented("task restart is not yet implemented".to_owned()))
}

#[derive(Debug, Deserialize, Validate)]
struct TaskIdPath {
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "id must not be empty"
    ))]
    id: String,
}
