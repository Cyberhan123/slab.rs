use std::sync::Arc;

use slab_app_core::context::AppState;
use slab_app_core::schemas::system::GpuStatusResponse;

#[tauri::command(async)]
pub async fn gpu_status(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<GpuStatusResponse, String> {
    Ok(state.services.system.gpu_status().await.into())
}
