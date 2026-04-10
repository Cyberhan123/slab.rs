use std::sync::Arc;

use slab_app_core::context::AppState;
use slab_app_core::schemas::ui_state::{
    UiStateDeleteResponse, UiStateValueResponse, UpdateUiStateRequest,
};

use crate::api::validation::{map_err, validate, validate_id};

#[tauri::command(async)]
pub async fn get_ui_state(
    state: tauri::State<'_, Arc<AppState>>,
    key: String,
) -> Result<UiStateValueResponse, String> {
    validate_id(&key)?;
    state.services.ui_state.get_ui_state(&key).await.map(Into::into).map_err(map_err)
}

#[tauri::command(async)]
pub async fn update_ui_state(
    state: tauri::State<'_, Arc<AppState>>,
    key: String,
    body: UpdateUiStateRequest,
) -> Result<UiStateValueResponse, String> {
    validate_id(&key)?;
    let body = validate(body)?;
    state
        .services
        .ui_state
        .update_ui_state(&key, body.into())
        .await
        .map(Into::into)
        .map_err(map_err)
}

#[tauri::command(async)]
pub async fn delete_ui_state(
    state: tauri::State<'_, Arc<AppState>>,
    key: String,
) -> Result<UiStateDeleteResponse, String> {
    validate_id(&key)?;
    state.services.ui_state.delete_ui_state(&key).await.map(Into::into).map_err(map_err)
}
