//! Generic task management endpoints.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::bounded_contexts::task_management::application::TaskApplicationService;
use crate::contexts::task::application::get_task_result_use_case::{
    GetTaskResultUseCase, TaskResultPort,
};
use crate::contexts::task::domain::TaskResult;
use crate::contexts::task::interface::http::mappers::task_mapper::to_task_result_response;
use crate::error::ServerError;
use crate::schemas::v1::task::{TaskResponse, TaskResultPayload, TaskTypeQuery};
use crate::state::TaskContext;

use super::V1State;

#[derive(OpenApi)]
#[openapi(
    paths(list_tasks, get_task, get_task_result, cancel_task, restart_task),
    components(schemas(TaskResponse, TaskResultPayload, TaskTypeQuery))
)]
pub struct TasksApi;

pub fn router() -> Router<Arc<V1State>> {
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
    State(context): State<Arc<TaskContext>>,
    Query(q): Query<TaskTypeQuery>,
) -> Result<Json<Vec<TaskResponse>>, ServerError> {
    let service = TaskApplicationService::new(context);
    let tasks = service.list_tasks(q.task_type.as_deref()).await?;
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
pub async fn get_task(
    State(context): State<Arc<TaskContext>>,
    Path(id): Path<String>,
) -> Result<Json<TaskResponse>, ServerError> {
    let service = TaskApplicationService::new(context);
    let task = service.get_task(&id).await?;
    Ok(Json(task))
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
    State(context): State<Arc<TaskContext>>,
    Path(id): Path<String>,
) -> Result<Json<TaskResultPayload>, ServerError> {
    let use_case = GetTaskResultUseCase::new(TaskResultRoutePort { context });
    let task_result = use_case.execute(id).await?;
    Ok(Json(to_task_result_response(task_result)))
}

struct TaskResultRoutePort {
    context: Arc<TaskContext>,
}

impl TaskResultPort for TaskResultRoutePort {
    fn get_task_result(
        &self,
        id: String,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<TaskResult, ServerError>> + Send + '_>,
    > {
        let context = Arc::clone(&self.context);
        Box::pin(async move {
            let service = TaskApplicationService::new(context);
            service.get_task_result(&id).await
        })
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
    State(context): State<Arc<TaskContext>>,
    Path(id): Path<String>,
) -> Result<Json<TaskResponse>, ServerError> {
    let service = TaskApplicationService::new(context);
    let task = service.cancel_task(&id).await?;
    Ok(Json(task))
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
    State(context): State<Arc<TaskContext>>,
    Path(id): Path<String>,
) -> Result<Json<TaskResponse>, ServerError> {
    let service = TaskApplicationService::new(context);
    service.validate_restartable(&id).await?;

    Err(ServerError::NotImplemented(
        "task restart is not yet implemented".to_owned(),
    ))
}
