use chrono::Utc;
use futures::{stream, StreamExt};
use slab_proto::convert;
use slab_types::inference::TextGenerationRequest;
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{ChatStreamChunk, ConversationMessage as DomainConversationMessage};
use crate::error::ServerError;
use crate::infra::rpc;

use super::GeneratedChatOutput;

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
    let request = TextGenerationRequest {
        prompt,
        system_prompt: None,
        max_tokens: Some(max_tokens),
        temperature: Some(temperature),
        top_p: None,
        session_key: session_id.map(str::to_owned),
        stream,
        options: Default::default(),
    };
    let grpc_request = convert::encode_chat_request(model.to_owned(), &request);

    let llama_channel = state.grpc().chat_channel().ok_or_else(|| {
        ServerError::BackendNotReady("llama gRPC endpoint is not configured".into())
    })?;

    if stream {
        let usage_guard =
            state.auto_unload().acquire_for_inference(super::LLAMA_BACKEND_ID).await.map_err(
                |error| ServerError::BackendNotReady(format!("llama backend not ready: {error}")),
            )?;

        let backend_stream = rpc::client::chat_stream(llama_channel.clone(), grpc_request.clone())
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
            .chain(stream::once(async { ChatStreamChunk::Data("[DONE]".into()) }))
            .map(move |item| {
                // Keep the usage guard alive for the whole SSE stream lifetime.
                let _keep_alive = &usage_guard;
                item
            });

        return Ok(GeneratedChatOutput::Stream(Box::pin(sse_stream)));
    }

    let _usage_guard =
        state.auto_unload().acquire_for_inference(super::LLAMA_BACKEND_ID).await.map_err(
            |error| ServerError::BackendNotReady(format!("llama backend not ready: {error}")),
        )?;

    let generated = rpc::client::chat(llama_channel, grpc_request)
        .await
        .map_err(|error| ServerError::Internal(format!("grpc chat failed: {error}")))?;

    Ok(GeneratedChatOutput::Text(generated))
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
