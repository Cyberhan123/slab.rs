use std::sync::Arc;

use slab_app_core::context::AppState;
use slab_app_core::domain::models::{ChatCompletionOutput, TextCompletionOutput};
use slab_app_core::error::AppCoreError;
use slab_app_core::schemas::chat::{
    ChatCompletionRequest, ChatCompletionResponse, ChatModelOption, CompletionRequest,
    CompletionResponse,
};

use crate::api::validation::{map_err, validate};

#[tauri::command(async)]
pub async fn list_chat_models(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<ChatModelOption>, String> {
    let items = state.services.chat.list_chat_models().await.map_err(map_err)?;
    Ok(items.into_iter().map(Into::into).collect())
}

#[tauri::command(async)]
pub async fn chat_completions(
    state: tauri::State<'_, Arc<AppState>>,
    req: ChatCompletionRequest,
) -> Result<ChatCompletionResponse, String> {
    let req = validate(req)?;
    let output = state.services.chat.create_chat_completion(req.into()).await.map_err(map_err)?;
    match output {
        ChatCompletionOutput::Json(response) => Ok(response.into()),
        ChatCompletionOutput::Stream(_) => Err(map_err(AppCoreError::NotImplemented(
            "streaming chat completions are not available over Tauri IPC; use /v1/chat/completions over HTTP".to_owned(),
        ))),
    }
}

#[tauri::command(async)]
pub async fn completions(
    state: tauri::State<'_, Arc<AppState>>,
    req: CompletionRequest,
) -> Result<CompletionResponse, String> {
    let req = validate(req)?;
    let output = state.services.chat.create_text_completion(req.into()).await.map_err(map_err)?;
    match output {
        TextCompletionOutput::Json(response) => Ok(response.into()),
        TextCompletionOutput::Stream(_) => Err(map_err(AppCoreError::NotImplemented(
            "streaming text completions are not available over Tauri IPC; use /v1/completions over HTTP".to_owned(),
        ))),
    }
}
