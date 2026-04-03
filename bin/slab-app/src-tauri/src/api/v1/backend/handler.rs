use std::sync::Arc;

use slab_app_core::context::AppState;
use slab_app_core::schemas::backend::{
    BackendListResponse, BackendStatusResponse, BackendTypeQuery, DownloadLibRequest,
};
use slab_app_core::schemas::tasks::OperationAcceptedResponse;

use crate::api::validation::{map_err, validate};

#[tauri::command(async)]
pub async fn backend_status(
    state: tauri::State<'_, Arc<AppState>>,
    query: BackendTypeQuery,
) -> Result<BackendStatusResponse, String> {
    let query = validate(query)?;
    Ok(state.services.backend.backend_status(query.into()).await.map_err(map_err)?.into())
}

#[tauri::command(async)]
pub async fn list_backends(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<BackendListResponse, String> {
    let backends = state.services.backend.list_backends().await.map_err(map_err)?;
    Ok(BackendListResponse { backends: backends.into_iter().map(Into::into).collect() })
}

#[tauri::command(async)]
pub async fn download_backend_lib(
    state: tauri::State<'_, Arc<AppState>>,
    req: DownloadLibRequest,
) -> Result<OperationAcceptedResponse, String> {
    let req = validate(req)?;
    Ok(state.services.backend.download_lib(req.into()).await.map_err(map_err)?.into())
}
