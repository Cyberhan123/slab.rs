use chrono::Utc;
use futures::{stream, StreamExt};
use slab_proto::convert;
use slab_types::inference::TextGenerationRequest;
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatStreamChunk, ConversationMessage as DomainConversationMessage, UnifiedModel,
};
use crate::error::ServerError;
use crate::infra::db::ModelStore;
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
    // Always pre-render a fallback prompt using the server-side static
    // template renderer.  We also send the structured messages with
    // `apply_chat_template = true` so the llama backend will try to apply
    // the model's own embedded chat template first.  If the embedded
    // template is absent or fails, the backend falls back to the
    // pre-rendered prompt automatically.
    let prompt_template = resolve_prompt_template(state, model).await?;
    let prompt = super::template::build_prompt(messages, prompt_template.as_deref());
    let request = TextGenerationRequest {
        prompt,
        system_prompt: None,
        chat_messages: messages.to_vec(),
        apply_chat_template: true,
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
        let completion_id_for_role = completion_id.clone();
        let model_name_for_role = model_name.clone();
        let completion_id_for_tokens = completion_id.clone();
        let model_name_for_tokens = model_name.clone();
        let completion_id_for_finish = completion_id.clone();
        let model_name_for_finish = model_name.clone();

        let role_chunk = stream::once(async move {
            ChatStreamChunk::Data(super::build_role_chunk(
                &completion_id_for_role,
                created_ts,
                &model_name_for_role,
            ))
        });
        let token_stream = backend_stream.filter_map(move |chunk| {
            let completion_id = completion_id_for_tokens.clone();
            let model_name = model_name_for_tokens.clone();
            async move {
                match chunk {
                    Ok(message) if !message.error.is_empty() => {
                        Some(ChatStreamChunk::Comment(message.error))
                    }
                    Ok(message) if message.done => None,
                    Ok(message) => Some(ChatStreamChunk::Data(super::build_chunk(
                        &completion_id,
                        created_ts,
                        &model_name,
                        &message.token,
                    ))),
                    Err(error) => Some(ChatStreamChunk::Comment(error.to_string())),
                }
            }
        });
        let finish_chunk = stream::once(async move {
            ChatStreamChunk::Data(super::build_finish_chunk(
                &completion_id_for_finish,
                created_ts,
                &model_name_for_finish,
                "stop",
            ))
        });

        let sse_stream = role_chunk
            .chain(token_stream)
            .chain(finish_chunk)
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

async fn resolve_prompt_template(
    state: &ModelState,
    model: &str,
) -> Result<Option<String>, ServerError> {
    let Some(record) = state.store().get_model(model).await? else {
        return Ok(None);
    };
    let model: UnifiedModel =
        record.try_into().map_err(|error: String| ServerError::Internal(error))?;
    Ok(model.spec.chat_template)
}
