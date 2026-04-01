use std::sync::Arc;

use slab_app_core::context::AppState;

/// Return `true` when the embedded core state is registered and reachable.
#[tauri::command(async)]
pub async fn health(state: tauri::State<'_, Arc<AppState>>) -> Result<bool, String> {
    let _ = state.inner();
    Ok(true)
}
