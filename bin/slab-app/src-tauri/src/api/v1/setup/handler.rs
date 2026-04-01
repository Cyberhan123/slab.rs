use std::sync::Arc;

use slab_app_core::context::AppState;
use slab_app_core::schemas::setup::{CompleteSetupRequest, SetupStatusResponse};
use slab_app_core::schemas::tasks::OperationAcceptedResponse;

use crate::api::validation::map_err;

#[tauri::command(async)]
pub async fn setup_status(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<SetupStatusResponse, String> {
    Ok(state.services.setup.environment_status().await.map_err(map_err)?.into())
}

#[tauri::command(async)]
pub async fn download_ffmpeg(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<OperationAcceptedResponse, String> {
    let op = state.services.setup.download_ffmpeg().await.map_err(map_err)?;
    Ok(OperationAcceptedResponse { operation_id: op.operation_id })
}

#[tauri::command(async)]
pub async fn complete_setup(
    state: tauri::State<'_, Arc<AppState>>,
    req: CompleteSetupRequest,
) -> Result<SetupStatusResponse, String> {
    Ok(state.services.setup.complete_setup(req.into()).await.map_err(map_err)?.into())
}
