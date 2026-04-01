use std::sync::Arc;

use slab_app_core::context::AppState;
use slab_app_core::schemas::session::{CreateSessionRequest, MessageResponse, SessionResponse};

use crate::api::validation::{map_err, validate, validate_id};

#[tauri::command(async)]
pub async fn list_sessions(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<SessionResponse>, String> {
    let sessions = state.services.session.list_sessions().await.map_err(map_err)?;
    Ok(sessions.into_iter().map(Into::into).collect())
}

#[tauri::command(async)]
pub async fn create_session(
    state: tauri::State<'_, Arc<AppState>>,
    req: CreateSessionRequest,
) -> Result<SessionResponse, String> {
    let req = validate(req)?;
    Ok(state.services.session.create_session(req.into()).await.map_err(map_err)?.into())
}

#[tauri::command(async)]
pub async fn delete_session(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<serde_json::Value, String> {
    validate_id(&id)?;
    state.services.session.delete_session(&id).await.map_err(map_err)
}

#[tauri::command(async)]
pub async fn list_session_messages(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<Vec<MessageResponse>, String> {
    validate_id(&id)?;
    let messages = state.services.session.list_session_messages(&id).await.map_err(map_err)?;
    Ok(messages.into_iter().map(Into::into).collect())
}
