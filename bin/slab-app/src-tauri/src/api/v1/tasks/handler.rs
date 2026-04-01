use std::sync::Arc;

use slab_app_core::context::AppState;
use slab_app_core::error::AppCoreError;
use slab_app_core::schemas::tasks::{TaskResponse, TaskResultPayload, TaskTypeQuery};

use crate::api::validation::{map_err, validate, validate_id};

#[tauri::command(async)]
pub async fn list_tasks(
    state: tauri::State<'_, Arc<AppState>>,
    query: TaskTypeQuery,
) -> Result<Vec<TaskResponse>, String> {
    let query = validate(query)?;
    let tasks = state
        .services
        .task_application
        .list_tasks(query.task_type.as_deref())
        .await
        .map_err(map_err)?;
    Ok(tasks.into_iter().map(Into::into).collect())
}

#[tauri::command(async)]
pub async fn get_task(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<TaskResponse, String> {
    validate_id(&id)?;
    Ok(state.services.task_application.get_task(&id).await.map_err(map_err)?.into())
}

#[tauri::command(async)]
pub async fn get_task_result(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<TaskResultPayload, String> {
    validate_id(&id)?;
    Ok(state.services.task_application.get_task_result(&id).await.map_err(map_err)?.into())
}

#[tauri::command(async)]
pub async fn cancel_task(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<TaskResponse, String> {
    validate_id(&id)?;
    Ok(state.services.task_application.cancel_task(&id).await.map_err(map_err)?.into())
}

#[tauri::command(async)]
pub async fn restart_task(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<TaskResponse, String> {
    validate_id(&id)?;
    state.services.task_application.validate_restartable(&id).await.map_err(map_err)?;
    Err(map_err(AppCoreError::NotImplemented("task restart is not yet implemented".to_owned())))
}
