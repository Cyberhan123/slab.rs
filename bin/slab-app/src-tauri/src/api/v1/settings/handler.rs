use std::sync::Arc;

use slab_app_core::context::AppState;
use slab_app_core::domain::models::{
    SettingPropertyView, SettingsDocumentView, UpdateSettingCommand,
};

use crate::api::validation::{map_err, validate_id};

#[tauri::command(async)]
pub async fn list_settings(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<SettingsDocumentView, String> {
    state.services.settings.list_settings().await.map_err(map_err)
}

#[tauri::command(async)]
pub async fn get_setting(
    state: tauri::State<'_, Arc<AppState>>,
    pmid: String,
) -> Result<SettingPropertyView, String> {
    validate_id(&pmid)?;
    state.services.settings.get_setting(&pmid).await.map_err(map_err)
}

#[tauri::command(async)]
pub async fn update_setting(
    state: tauri::State<'_, Arc<AppState>>,
    pmid: String,
    body: UpdateSettingCommand,
) -> Result<SettingPropertyView, String> {
    validate_id(&pmid)?;
    state.services.settings.update_setting(&pmid, body).await.map_err(map_err)
}
