use std::sync::Arc;

use slab_app_core::context::AppState;
use slab_app_core::schemas::chat::ChatModelOption;

use crate::api::validation::map_err;

#[tauri::command(async)]
pub async fn list_chat_models(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<ChatModelOption>, String> {
    let items = state.services.chat.list_chat_models().await.map_err(map_err)?;
    Ok(items.into_iter().map(Into::into).collect())
}
