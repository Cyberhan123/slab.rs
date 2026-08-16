//! OpenAI-compatible chat completion — cloud route.
//!
//! This module keeps only the chat wire-format assembly (SSE chunk chain +
//! [`GeneratedChatOutput`]). The underlying genai calls (catalog resolution, HTTP
//! request/stream, tracing, redaction, token estimation) have been pushed down into
//! [`crate::domain::services::llm::cloud`] / [`crate::domain::services::llm`], shared by
//! chat and agent/response.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use futures::{StreamExt, stream};
use tracing::warn;
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatStreamChunk, ConversationMessage as DomainConversationMessage, TextGenerationResponse,
};
use crate::domain::services::llm::cloud::{
    CloudChatRequestConfig, CloudDelta, cloud_chat_completion, cloud_chat_stream,
    render_messages_for_usage, resolve_cloud_model,
};
use crate::domain::services::llm::{build_estimated_usage, finish_reason_from_token_budget};
use crate::error::AppCoreError;

use super::GeneratedChatOutput;

pub(super) async fn create_chat_completion(
    state: &ModelState,
    requested_model: &str,
    messages: &[DomainConversationMessage],
    config: CloudChatRequestConfig,
) -> Result<GeneratedChatOutput, AppCoreError> {
    let target = resolve_cloud_model(state, requested_model).await?;
    let trace_http = state.pmid().config().server.cloud_http_trace;

    if config.stream && !config.tools.is_empty() {
        warn!(
            provider_id = %target.provider_id,
            provider_name = %target.provider_name,
            remote_model = %target.remote_model,
            "cloud tool streaming is falling back to non-streaming so native tool call chunks are preserved"
        );
    }

    if config.stream && config.tools.is_empty() {
        let max_tokens = config.max_tokens;
        let include_usage = config.include_usage;
        let backend_stream = cloud_chat_stream(&target, messages, config, trace_http).await?;
        let completion_id = format!("chatcmpl-{}", Uuid::new_v4());
        let created_ts = chrono::Utc::now().timestamp();
        let model_name = requested_model.to_owned();
        let completion_id_for_tokens = completion_id.clone();
        let model_name_for_tokens = model_name.clone();
        let completion_id_for_role = completion_id.clone();
        let model_name_for_role = model_name.clone();
        let completion_id_for_finish = completion_id.clone();
        let model_name_for_finish = model_name.clone();
        let completion_id_for_usage = completion_id.clone();
        let model_name_for_usage = model_name.clone();
        let prompt_for_usage = render_messages_for_usage(messages);

        let error_flag = Arc::new(AtomicBool::new(false));
        let completion_tokens = Arc::new(AtomicU32::new(0));

        let role_chunk = stream::once(async move {
            super::build_role_chunk(&completion_id_for_role, created_ts, &model_name_for_role)
        });

        let token_stream_error_flag = Arc::clone(&error_flag);
        let token_stream_completion_tokens = Arc::clone(&completion_tokens);
        let token_stream = backend_stream.map(move |chunk| -> ChatStreamChunk {
            match chunk {
                Ok(CloudDelta::Content(token)) => {
                    token_stream_completion_tokens.fetch_add(1, Ordering::SeqCst);
                    super::build_chunk(
                        &completion_id_for_tokens,
                        created_ts,
                        &model_name_for_tokens,
                        &token,
                    )
                }
                Ok(CloudDelta::Reasoning(token)) => super::build_reasoning_chunk(
                    &completion_id_for_tokens,
                    created_ts,
                    &model_name_for_tokens,
                    &token,
                ),
                Err(error) => {
                    token_stream_error_flag.store(true, Ordering::SeqCst);
                    super::build_error_chunk(&error.to_string())
                }
            }
        });

        let finish_chunk_error_flag = Arc::clone(&error_flag);
        let finish_chunk_completion_tokens = Arc::clone(&completion_tokens);
        let finish_chunk = stream::once(async move {
            if finish_chunk_error_flag.load(Ordering::SeqCst) {
                None
            } else {
                let finish_reason = finish_reason_from_token_budget(
                    finish_chunk_completion_tokens.load(Ordering::SeqCst),
                    max_tokens,
                );
                Some(super::build_finish_chunk(
                    &completion_id_for_finish,
                    created_ts,
                    &model_name_for_finish,
                    &finish_reason,
                ))
            }
        });

        let usage_chunk_error_flag = Arc::clone(&error_flag);
        let usage_chunk_completion_tokens = Arc::clone(&completion_tokens);
        let usage_chunk = stream::once(async move {
            if !include_usage || usage_chunk_error_flag.load(Ordering::SeqCst) {
                None
            } else {
                let usage = build_estimated_usage(
                    &prompt_for_usage,
                    "",
                    Some(usage_chunk_completion_tokens.load(Ordering::SeqCst)),
                );
                Some(super::build_usage_chunk(
                    &completion_id_for_usage,
                    created_ts,
                    &model_name_for_usage,
                    &usage,
                ))
            }
        });

        let sse_stream = role_chunk
            .chain(token_stream)
            .chain(finish_chunk.filter_map(futures::future::ready))
            .chain(usage_chunk.filter_map(futures::future::ready))
            .chain(stream::once(async { "[DONE]".to_owned() }));

        return Ok(GeneratedChatOutput::Stream(Box::pin(sse_stream)));
    }

    let generated = cloud_chat_completion(&target, messages, config, trace_http).await?;
    Ok(GeneratedChatOutput::Text(generated))
}

pub(super) async fn create_text_completion(
    state: &ModelState,
    requested_model: &str,
    prompt: &str,
    config: CloudChatRequestConfig,
) -> Result<TextGenerationResponse, AppCoreError> {
    let target = resolve_cloud_model(state, requested_model).await?;
    let messages = vec![DomainConversationMessage {
        role: "user".to_owned(),
        content: slab_types::chat::ConversationMessageContent::Text(prompt.to_owned()),
        name: None,
        tool_call_id: None,
        tool_calls: Vec::new(),
    }];

    cloud_chat_completion(&target, &messages, config, state.pmid().config().server.cloud_http_trace)
        .await
}
