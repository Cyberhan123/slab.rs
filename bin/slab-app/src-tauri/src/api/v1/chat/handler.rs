use std::sync::Arc;

use futures::StreamExt as _;
use slab_app_core::context::AppState;
use slab_app_core::domain::models::{ChatCompletionOutput, ChatStreamChunk, TextCompletionOutput};
use slab_app_core::error::AppCoreError;
use slab_app_core::schemas::chat::{
    ChatCompletionRequest, ChatCompletionResponse, ChatModelOption, CompletionRequest,
    CompletionResponse,
};
use tauri::ipc::Channel;

use crate::api::validation::{map_err, validate};

/// Events emitted on the streaming channel for chat and text completions.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatStreamEvent {
    /// A raw SSE data payload (OpenAI-compatible JSON string).
    Data { data: String },
    /// Signals that the stream has finished successfully.
    Done,
    /// Signals a non-recoverable stream error.
    Error { message: String },
}

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
    let mut req = validate(req)?;
    req.stream = false;
    let output = state.services.chat.create_chat_completion(req.into()).await.map_err(map_err)?;
    match output {
        ChatCompletionOutput::Json(response) => Ok(response.into()),
        ChatCompletionOutput::Stream(_) => Err(map_err(AppCoreError::NotImplemented(
            "unexpected stream output from non-streaming chat completion request".to_owned(),
        ))),
    }
}

/// Stream chat completions over a Tauri IPC channel.
///
/// Each chunk is emitted as a [`ChatStreamEvent::Data`] message containing
/// the raw OpenAI-compatible SSE payload string.  When the model finishes
/// (or an error occurs) a final [`ChatStreamEvent::Done`] or
/// [`ChatStreamEvent::Error`] message is emitted.
#[tauri::command(async)]
pub async fn chat_completions_stream(
    state: tauri::State<'_, Arc<AppState>>,
    req: ChatCompletionRequest,
    on_event: Channel<ChatStreamEvent>,
) -> Result<(), String> {
    let mut req = validate(req)?;
    req.stream = true;
    let output = state.services.chat.create_chat_completion(req.into()).await.map_err(map_err)?;

    match output {
        ChatCompletionOutput::Json(response) => {
            let payload = serde_json::to_string(&ChatCompletionResponse::from(response))
                .map_err(|e| e.to_string())?;
            let _ = on_event.send(ChatStreamEvent::Data { data: payload });
            let _ = on_event.send(ChatStreamEvent::Done);
        }
        ChatCompletionOutput::Stream(mut stream) => {
            while let Some(chunk) = stream.next().await {
                let ChatStreamChunk::Data(data) = chunk;
                if on_event.send(ChatStreamEvent::Data { data }).is_err() {
                    break;
                }
            }
            let _ = on_event.send(ChatStreamEvent::Done);
        }
    }

    Ok(())
}

#[tauri::command(async)]
pub async fn completions(
    state: tauri::State<'_, Arc<AppState>>,
    req: CompletionRequest,
) -> Result<CompletionResponse, String> {
    let mut req = validate(req)?;
    req.stream = false;
    let output = state.services.chat.create_text_completion(req.into()).await.map_err(map_err)?;
    match output {
        TextCompletionOutput::Json(response) => Ok(response.into()),
        TextCompletionOutput::Stream(_) => Err(map_err(AppCoreError::NotImplemented(
            "unexpected stream output from non-streaming text completion request".to_owned(),
        ))),
    }
}
