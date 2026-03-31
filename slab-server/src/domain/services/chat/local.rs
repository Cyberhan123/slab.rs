use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use chrono::Utc;
use futures::{StreamExt, stream};
use slab_proto::convert;
use slab_types::inference::{TextGenerationRequest, TextGenerationUsage};
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatStreamChunk, ConversationMessage as DomainConversationMessage, UnifiedModel,
};
use crate::error::ServerError;
use crate::infra::db::ModelStore;
use crate::infra::rpc;

use super::GeneratedChatOutput;

#[derive(Debug, Clone, Default)]
struct LocalStreamTerminalMetadata {
    finish_reason: Option<String>,
    usage: Option<TextGenerationUsage>,
}

#[derive(Debug, Clone)]
pub(super) struct LocalChatRequestConfig {
    pub(super) session_id: Option<String>,
    pub(super) max_tokens: u32,
    pub(super) temperature: f32,
    pub(super) top_p: Option<f32>,
    pub(super) grammar: Option<String>,
    pub(super) grammar_json: bool,
    pub(super) stream: bool,
    pub(super) include_usage: bool,
}

#[derive(Debug, Clone)]
pub(super) struct LocalTextRequestConfig {
    pub(super) max_tokens: u32,
    pub(super) temperature: f32,
    pub(super) top_p: Option<f32>,
    pub(super) grammar: Option<String>,
    pub(super) grammar_json: bool,
}

pub(super) async fn create_chat_completion(
    state: &ModelState,
    model: &str,
    messages: &[DomainConversationMessage],
    config: LocalChatRequestConfig,
) -> Result<GeneratedChatOutput, ServerError> {
    let prompt_template_context = resolve_prompt_template_context(state, model).await?;
    let prompt = super::template::build_prompt(messages, prompt_template_context.as_ref());
    let request = TextGenerationRequest {
        prompt: prompt.clone(),
        system_prompt: None,
        chat_messages: messages.to_vec(),
        apply_chat_template: true,
        max_tokens: Some(config.max_tokens),
        temperature: Some(config.temperature),
        top_p: config.top_p,
        session_key: config.session_id.clone(),
        stream: config.stream,
        grammar: config.grammar.clone(),
        grammar_json: config.grammar_json,
        ..Default::default()
    };
    let grpc_request = convert::encode_chat_request(model.to_owned(), &request);

    let llama_channel = state.grpc().chat_channel().ok_or_else(|| {
        ServerError::BackendNotReady("llama gRPC endpoint is not configured".into())
    })?;

    if config.stream {
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
        let completion_id_for_usage = completion_id.clone();
        let model_name_for_usage = model_name.clone();
        let prompt_for_usage = prompt.clone();

        let error_flag = Arc::new(AtomicBool::new(false));
        let completion_tokens = Arc::new(AtomicU32::new(0));
        let terminal_metadata = Arc::new(Mutex::new(LocalStreamTerminalMetadata::default()));

        let role_chunk = stream::once(async move {
            ChatStreamChunk::Data(super::build_role_chunk(
                &completion_id_for_role,
                created_ts,
                &model_name_for_role,
            ))
        });

        let token_stream_error_flag = Arc::clone(&error_flag);
        let token_stream_completion_tokens = Arc::clone(&completion_tokens);
        let token_stream_terminal_metadata = Arc::clone(&terminal_metadata);
        let token_stream = backend_stream.filter_map(move |chunk| {
            let completion_id = completion_id_for_tokens.clone();
            let model_name = model_name_for_tokens.clone();
            let error_flag = Arc::clone(&token_stream_error_flag);
            let completion_tokens = Arc::clone(&token_stream_completion_tokens);
            let terminal_metadata = Arc::clone(&token_stream_terminal_metadata);
            async move {
                match chunk {
                    Ok(message) if !message.error.is_empty() => {
                        error_flag.store(true, Ordering::SeqCst);
                        Some(ChatStreamChunk::Data(super::build_error_chunk(&message.error)))
                    }
                    Ok(message) => {
                        let decoded = convert::decode_chat_stream_chunk(&message);
                        if decoded.done {
                            let mut terminal = terminal_metadata
                                .lock()
                                .expect("local chat terminal metadata lock poisoned");
                            if decoded.finish_reason.is_some() {
                                terminal.finish_reason = decoded.finish_reason;
                            }
                            if decoded.usage.is_some() {
                                terminal.usage = decoded.usage;
                            }
                            None
                        } else if decoded.delta.is_empty() {
                            None
                        } else {
                            completion_tokens.fetch_add(1, Ordering::SeqCst);
                            Some(ChatStreamChunk::Data(super::build_chunk(
                                &completion_id,
                                created_ts,
                                &model_name,
                                &decoded.delta,
                            )))
                        }
                    }
                    Err(error) => {
                        error_flag.store(true, Ordering::SeqCst);
                        Some(ChatStreamChunk::Data(super::build_error_chunk(&error.to_string())))
                    }
                }
            }
        });

        let finish_chunk_error_flag = Arc::clone(&error_flag);
        let finish_chunk_completion_tokens = Arc::clone(&completion_tokens);
        let finish_chunk_terminal_metadata = Arc::clone(&terminal_metadata);
        let finish_chunk = stream::once(async move {
            if finish_chunk_error_flag.load(Ordering::SeqCst) {
                None
            } else {
                let finish_reason = finish_chunk_terminal_metadata
                    .lock()
                    .expect("local chat terminal metadata lock poisoned")
                    .finish_reason
                    .clone()
                    .unwrap_or_else(|| {
                        super::finish_reason_from_token_budget(
                            finish_chunk_completion_tokens.load(Ordering::SeqCst),
                            config.max_tokens,
                        )
                    });
                Some(ChatStreamChunk::Data(super::build_finish_chunk(
                    &completion_id_for_finish,
                    created_ts,
                    &model_name_for_finish,
                    &finish_reason,
                )))
            }
        })
        .filter_map(futures::future::ready);

        let usage_chunk_error_flag = Arc::clone(&error_flag);
        let usage_chunk_completion_tokens = Arc::clone(&completion_tokens);
        let usage_chunk_terminal_metadata = Arc::clone(&terminal_metadata);
        let usage_chunk = stream::once(async move {
            if !config.include_usage || usage_chunk_error_flag.load(Ordering::SeqCst) {
                None
            } else {
                let usage = usage_chunk_terminal_metadata
                    .lock()
                    .expect("local chat terminal metadata lock poisoned")
                    .usage
                    .clone()
                    .unwrap_or_else(|| {
                        super::build_estimated_usage(
                            &prompt_for_usage,
                            "",
                            Some(usage_chunk_completion_tokens.load(Ordering::SeqCst)),
                        )
                    });
                Some(ChatStreamChunk::Data(super::build_usage_chunk(
                    &completion_id_for_usage,
                    created_ts,
                    &model_name_for_usage,
                    &usage,
                )))
            }
        })
        .filter_map(futures::future::ready);

        let sse_stream = role_chunk
            .chain(token_stream)
            .chain(finish_chunk)
            .chain(usage_chunk)
            .chain(stream::once(async { ChatStreamChunk::Data("[DONE]".into()) }))
            .map(move |item| {
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
    let mut response = convert::decode_chat_response(&generated);

    let usage = response.usage.clone().unwrap_or_else(|| {
        super::build_estimated_usage(&prompt, &response.text, response.tokens_used)
    });
    response.tokens_used.get_or_insert(usage.completion_tokens);
    response.usage = Some(usage.clone());
    response.finish_reason.get_or_insert_with(|| {
        super::finish_reason_from_token_budget(usage.completion_tokens, config.max_tokens)
    });

    Ok(GeneratedChatOutput::Text(response))
}

pub(super) async fn create_text_completion(
    state: &ModelState,
    model: &str,
    prompt: &str,
    config: LocalTextRequestConfig,
) -> Result<slab_types::inference::TextGenerationResponse, ServerError> {
    let request = TextGenerationRequest {
        prompt: prompt.to_owned(),
        system_prompt: None,
        chat_messages: Vec::new(),
        apply_chat_template: false,
        max_tokens: Some(config.max_tokens),
        temperature: Some(config.temperature),
        top_p: config.top_p,
        stream: false,
        grammar: config.grammar,
        grammar_json: config.grammar_json,
        ..Default::default()
    };
    let grpc_request = convert::encode_chat_request(model.to_owned(), &request);

    let llama_channel = state.grpc().chat_channel().ok_or_else(|| {
        ServerError::BackendNotReady("llama gRPC endpoint is not configured".into())
    })?;

    let _usage_guard =
        state.auto_unload().acquire_for_inference(super::LLAMA_BACKEND_ID).await.map_err(
            |error| ServerError::BackendNotReady(format!("llama backend not ready: {error}")),
        )?;

    let generated = rpc::client::chat(llama_channel, grpc_request)
        .await
        .map_err(|error| ServerError::Internal(format!("grpc chat failed: {error}")))?;
    let mut response = convert::decode_chat_response(&generated);

    let usage = response.usage.clone().unwrap_or_else(|| {
        super::build_estimated_usage(prompt, &response.text, response.tokens_used)
    });
    response.tokens_used.get_or_insert(usage.completion_tokens);
    response.usage = Some(usage.clone());
    response.finish_reason.get_or_insert_with(|| {
        super::finish_reason_from_token_budget(usage.completion_tokens, config.max_tokens)
    });

    Ok(response)
}

async fn resolve_prompt_template_context(
    state: &ModelState,
    model: &str,
) -> Result<Option<super::template::PromptTemplateContext>, ServerError> {
    let Some(record) = state.store().get_model(model).await? else {
        return Ok(None);
    };
    let model: UnifiedModel =
        record.try_into().map_err(|error: String| ServerError::Internal(error))?;
    Ok(Some(super::template::PromptTemplateContext::from_model(&model)))
}
