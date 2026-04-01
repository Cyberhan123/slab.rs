use std::sync::Arc;

use slab_app_core::context::AppState;
use slab_app_core::schemas::models::{
    CreateModelRequest, DownloadModelRequest, ImportModelConfigRequest, ListAvailableQuery,
    ListModelsQuery, LoadModelRequest, ModelStatusResponse, SwitchModelRequest,
    UnifiedModelResponse, UnloadModelRequest, UpdateModelRequest,
};
use slab_app_core::schemas::tasks::OperationAcceptedResponse;

use crate::api::validation::{map_err, validate, validate_id};

#[tauri::command(async)]
pub async fn list_models(
    state: tauri::State<'_, Arc<AppState>>,
    query: Option<ListModelsQuery>,
) -> Result<Vec<UnifiedModelResponse>, String> {
    let filter = query.unwrap_or_default().into();
    let models = state.services.model.list_models(filter).await.map_err(map_err)?;
    Ok(models.into_iter().map(Into::into).collect())
}

#[tauri::command(async)]
pub async fn create_model(
    state: tauri::State<'_, Arc<AppState>>,
    req: CreateModelRequest,
) -> Result<UnifiedModelResponse, String> {
    let req = validate(req)?;
    Ok(state.services.model.create_model(req.into()).await.map_err(map_err)?.into())
}

#[tauri::command(async)]
pub async fn import_model_config(
    state: tauri::State<'_, Arc<AppState>>,
    req: ImportModelConfigRequest,
) -> Result<UnifiedModelResponse, String> {
    let req = validate(req)?;
    Ok(state.services.model.import_model_config(req.into()).await.map_err(map_err)?.into())
}

#[tauri::command(async)]
pub async fn get_model(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<UnifiedModelResponse, String> {
    validate_id(&id)?;
    Ok(state.services.model.get_model(&id).await.map_err(map_err)?.into())
}

#[tauri::command(async)]
pub async fn update_model(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
    req: UpdateModelRequest,
) -> Result<UnifiedModelResponse, String> {
    validate_id(&id)?;
    Ok(state.services.model.update_model(&id, req.into()).await.map_err(map_err)?.into())
}

#[tauri::command(async)]
pub async fn delete_model(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<serde_json::Value, String> {
    validate_id(&id)?;
    let view = state.services.model.delete_model(&id).await.map_err(map_err)?;
    Ok(serde_json::json!({ "id": view.id, "status": view.status }))
}

#[tauri::command(async)]
pub async fn load_model(
    state: tauri::State<'_, Arc<AppState>>,
    req: LoadModelRequest,
) -> Result<ModelStatusResponse, String> {
    let req = validate(req)?;
    Ok(state.services.model.load_model(req.into()).await.map_err(map_err)?.into())
}

#[tauri::command(async)]
pub async fn unload_model(
    state: tauri::State<'_, Arc<AppState>>,
    req: UnloadModelRequest,
) -> Result<ModelStatusResponse, String> {
    let req = validate(req)?;
    Ok(state.services.model.unload_model(req.into()).await.map_err(map_err)?.into())
}

#[tauri::command(async)]
pub async fn switch_model(
    state: tauri::State<'_, Arc<AppState>>,
    req: SwitchModelRequest,
) -> Result<ModelStatusResponse, String> {
    let req = validate(req)?;
    Ok(state.services.model.switch_model(req.into()).await.map_err(map_err)?.into())
}

#[tauri::command(async)]
pub async fn download_model(
    state: tauri::State<'_, Arc<AppState>>,
    req: DownloadModelRequest,
) -> Result<OperationAcceptedResponse, String> {
    let req = validate(req)?;
    Ok(state.services.model.download_model(req.into()).await.map_err(map_err)?.into())
}

#[tauri::command(async)]
pub async fn list_available_models(
    state: tauri::State<'_, Arc<AppState>>,
    query: ListAvailableQuery,
) -> Result<serde_json::Value, String> {
    let query = validate(query)?;
    let response =
        state.services.model.list_available_models(query.into()).await.map_err(map_err)?;
    Ok(serde_json::json!({ "repo_id": response.repo_id, "files": response.files }))
}
