use chrono::Utc;
use futures::{stream, StreamExt};
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatModelOption, ChatModelSource, ChatStreamChunk,
    ConversationMessage as DomainConversationMessage,
};
use crate::error::ServerError;
use crate::infra::db::{ModelStore, TaskRecord, TaskStore};
use crate::infra::rpc::{self, pb};

use super::GeneratedChatOutput;

pub(super) async fn list_chat_models(
    state: &ModelState,
) -> Result<Vec<ChatModelOption>, ServerError> {
    let local_models = state.store().list_models().await?;
    let download_tasks = state.store().list_tasks(Some("model_download")).await?;
    let pending_by_model = pending_download_map(download_tasks);

    Ok(local_models
        .into_iter()
        .filter(|model| {
            model
                .backend_ids
                .iter()
                .any(|backend| backend == super::LLAMA_BACKEND_ID)
        })
        .map(|model| ChatModelOption {
            id: model.id.clone(),
            display_name: model.display_name,
            source: ChatModelSource::Local,
            provider_id: None,
            provider_name: None,
            backend_id: Some(super::LLAMA_BACKEND_ID.to_owned()),
            downloaded: model.local_path.is_some(),
            pending: pending_by_model.contains_key(&model.id),
        })
        .collect())
}

pub(super) async fn create_chat_completion(
    state: &ModelState,
    model: &str,
    session_id: Option<&str>,
    messages: &[DomainConversationMessage],
    max_tokens: u32,
    temperature: f32,
    stream: bool,
) -> Result<GeneratedChatOutput, ServerError> {
    let prompt = build_prompt(messages);
    let request = pb::ChatRequest {
        prompt,
        model: model.to_owned(),
        max_tokens,
        temperature,
        session_key: session_id.unwrap_or_default().to_owned(),
    };

    let llama_channel = state.grpc().chat_channel().ok_or_else(|| {
        ServerError::BackendNotReady("llama gRPC endpoint is not configured".into())
    })?;

    if stream {
        let usage_guard = state
            .auto_unload()
            .acquire_for_inference(super::LLAMA_BACKEND_ID)
            .await
            .map_err(|error| {
                ServerError::BackendNotReady(format!("llama backend not ready: {error}"))
            })?;

        let backend_stream = rpc::client::chat_stream(llama_channel.clone(), request.clone())
            .await
            .map_err(|error| ServerError::Internal(format!("grpc chat stream failed: {error}")))?;

        let completion_id = format!("chatcmpl-{}", Uuid::new_v4());
        let created_ts = Utc::now().timestamp();
        let model_name = model.to_owned();

        let token_stream = backend_stream.map(move |chunk| -> ChatStreamChunk {
            match chunk {
                Ok(message) if !message.error.is_empty() => ChatStreamChunk::Comment(message.error),
                Ok(message) if message.done => ChatStreamChunk::Comment("done".into()),
                Ok(message) => ChatStreamChunk::Data(super::build_chunk(
                    &completion_id,
                    created_ts,
                    &model_name,
                    &message.token,
                )),
                Err(error) => ChatStreamChunk::Comment(error.to_string()),
            }
        });

        let sse_stream = token_stream
            .chain(stream::once(async {
                ChatStreamChunk::Data("[DONE]".into())
            }))
            .map(move |item| {
                // Keep the usage guard alive for the whole SSE stream lifetime.
                let _keep_alive = &usage_guard;
                item
            });

        return Ok(GeneratedChatOutput::Stream(Box::pin(sse_stream)));
    }

    let _usage_guard = state
        .auto_unload()
        .acquire_for_inference(super::LLAMA_BACKEND_ID)
        .await
        .map_err(|error| {
            ServerError::BackendNotReady(format!("llama backend not ready: {error}"))
        })?;

    let generated = rpc::client::chat(llama_channel, request)
        .await
        .map_err(|error| ServerError::Internal(format!("grpc chat failed: {error}")))?;

    Ok(GeneratedChatOutput::Text(generated))
}

fn pending_download_map(tasks: Vec<TaskRecord>) -> std::collections::HashMap<String, TaskRecord> {
    let mut pending_by_model: std::collections::HashMap<String, TaskRecord> =
        std::collections::HashMap::new();
    for task in tasks {
        if !matches!(task.status.as_str(), "pending" | "running") {
            continue;
        }

        let Some(model_id) = task.model_id.clone() else {
            continue;
        };

        let replace = pending_by_model
            .get(&model_id)
            .map(|current| task.updated_at > current.updated_at)
            .unwrap_or(true);
        if replace {
            pending_by_model.insert(model_id, task);
        }
    }
    pending_by_model
}

/// Build the local llama prompt from merged message history.
fn build_prompt(messages: &[DomainConversationMessage]) -> String {
    let mut parts: Vec<String> = messages
        .iter()
        .map(|message| format!("{}: {}", capitalize_role(&message.role), message.content))
        .collect();
    parts.push("Assistant:".into());
    parts.join("\n")
}

fn capitalize_role(role: &str) -> &str {
    match role {
        "user" => "User",
        "assistant" => "Assistant",
        "system" => "System",
        other => other,
    }
}
