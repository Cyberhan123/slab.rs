use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use futures::{StreamExt, stream};
use slab_agent_tracing::record_json_from_context;
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatReasoningEffort, ChatVerbosity, ConversationMessage as DomainConversationMessage,
    StructuredOutput, TextGenerationChunk, TextGenerationResponse, TextGenerationUsage,
    TextPromptTokensDetails,
};
use crate::domain::ports::{
    RuntimeTextGenerationChunk, RuntimeTextGenerationRequest, RuntimeTextGenerationResponse,
    RuntimeTextGenerationUsage,
};
use crate::domain::services::model;
use crate::error::AppCoreError;

use super::GeneratedChatOutput;

mod reasoning;

use reasoning::{
    ContentStopState, apply_local_reasoning_controls, apply_local_reasoning_controls_to_prompt,
    attach_reasoning_metadata, local_reasoning_guidance, merge_stop_sequences,
    reasoning_content_from_metadata, reasoning_is_disabled, route_stream_delta,
    suppress_reasoning_output, trim_trailing_stop_markers,
};

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
    pub(super) top_k: Option<i32>,
    pub(super) min_p: Option<f32>,
    pub(super) presence_penalty: Option<f32>,
    pub(super) repetition_penalty: Option<f32>,
    pub(super) reasoning_effort: Option<ChatReasoningEffort>,
    pub(super) verbosity: Option<ChatVerbosity>,
    pub(super) gbnf: Option<String>,
    pub(super) structured_output: Option<StructuredOutput>,
    pub(super) tools: Vec<slab_proto::openai::FunctionTool>,
    pub(super) stop: Vec<String>,
    pub(super) agent_trace: Option<slab_agent_tracing::AgentTraceContext>,
    pub(super) stream: bool,
    pub(super) include_usage: bool,
}

#[derive(Debug, Clone)]
pub(super) struct LocalTextRequestConfig {
    pub(super) max_tokens: u32,
    pub(super) temperature: f32,
    pub(super) top_p: Option<f32>,
    pub(super) top_k: Option<i32>,
    pub(super) min_p: Option<f32>,
    pub(super) presence_penalty: Option<f32>,
    pub(super) repetition_penalty: Option<f32>,
    pub(super) reasoning_effort: Option<ChatReasoningEffort>,
    pub(super) verbosity: Option<ChatVerbosity>,
    pub(super) gbnf: Option<String>,
    pub(super) structured_output: Option<StructuredOutput>,
}

pub(super) async fn create_chat_completion(
    state: &ModelState,
    model: &str,
    messages: &[DomainConversationMessage],
    config: LocalChatRequestConfig,
) -> Result<GeneratedChatOutput, AppCoreError> {
    let prompt_profile = model::resolve_local_chat_prompt_profile(state, model).await?;
    let backend_id = prompt_profile.backend_id;

    // When the Jinja chat template natively references `enable_thinking` (e.g.
    // Qwen3, DeepSeek-R1), it already controls thinking behaviour via the
    // template variable.  Skip injecting an extra system-level reasoning
    // guidance message; it can confuse smaller models and conflict with the
    // template's own thinking protocol.
    let native_thinking =
        super::template::template_supports_thinking(prompt_profile.chat_template_source.as_deref());
    let injected_guidance = if native_thinking {
        None
    } else {
        local_reasoning_guidance(config.reasoning_effort, config.verbosity)
    };
    let request_messages = if native_thinking {
        messages.to_vec()
    } else {
        apply_local_reasoning_controls(messages, config.reasoning_effort, config.verbosity)
    };
    if let Some(trace_context) = config.agent_trace.as_ref() {
        record_json_from_context(
            trace_context,
            "slab-app-core",
            "local_reasoning_policy_injected",
            serde_json::json!({
                "native_thinking": native_thinking,
                "injected": injected_guidance.is_some(),
                "guidance": injected_guidance,
                "reasoning_effort": config.reasoning_effort,
                "verbosity": config.verbosity,
            }),
        );
    }

    let prompt = super::template::build_prompt(
        &request_messages,
        prompt_profile.chat_template_source.as_deref(),
        config.reasoning_effort,
        &config.tools,
    )?;
    let effective_stop = merge_stop_sequences(
        &config.stop,
        &super::template::default_stop_sequences(prompt_profile.chat_template_source.as_deref()),
    );
    let trailing_stop_markers =
        super::template::trailing_stop_markers(prompt_profile.chat_template_source.as_deref());
    let gbnf = super::gbnf::resolve_effective_gbnf(
        config.gbnf.as_deref(),
        config.structured_output.as_ref(),
        prompt_profile.default_gbnf.as_deref(),
    )?;
    tracing::debug!(
        prompt_tail = &prompt[prompt.len().saturating_sub(120)..],
        native_thinking,
        stop_count = effective_stop.len(),
        "local chat prompt rendered"
    );
    if let Some(trace_context) = config.agent_trace.as_ref() {
        record_json_from_context(
            trace_context,
            "slab-app-core",
            "local_prompt_rendered",
            serde_json::json!({
                "model": model,
                "messages": request_messages,
                "prompt": prompt,
                "native_thinking": native_thinking,
                "chat_template_source": prompt_profile.chat_template_source,
                "tools": config.tools,
                "stop_sequences": effective_stop,
                "trailing_stop_markers": trailing_stop_markers,
                "gbnf": gbnf,
            }),
        );
    }
    let request = RuntimeTextGenerationRequest {
        backend_id: Some(backend_id),
        model: model.to_owned(),
        prompt: prompt.clone(),
        system_prompt: None,
        max_tokens: Some(config.max_tokens),
        temperature: Some(config.temperature),
        top_p: config.top_p,
        top_k: config.top_k,
        min_p: config.min_p,
        presence_penalty: config.presence_penalty,
        repetition_penalty: config.repetition_penalty,
        session_key: config.session_id.clone(),
        stream: config.stream,
        gbnf,
        stop_sequences: effective_stop.clone(),
        agent_trace: config.agent_trace.clone(),
    };
    if let Some(trace_context) = config.agent_trace.as_ref() {
        record_json_from_context(
            trace_context,
            "slab-app-core",
            "runtime_request",
            runtime_request_payload(&request),
        );
    }

    if config.stream {
        let usage_guard =
            state.auto_unload().acquire_for_inference(backend_id).await.map_err(|error| {
                AppCoreError::BackendNotReady(format!(
                    "{} backend not ready: {error}",
                    backend_id.canonical_id()
                ))
            })?;

        let backend_stream = state.runtime().chat_stream(request.clone()).await?;

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
        let reasoning_disabled = reasoning_is_disabled(config.reasoning_effort);

        let role_chunk = stream::once(async move {
            super::build_role_chunk(&completion_id_for_role, created_ts, &model_name_for_role)
        });

        let token_stream_error_flag = Arc::clone(&error_flag);
        let token_stream_completion_tokens = Arc::clone(&completion_tokens);
        let token_stream_terminal_metadata = Arc::clone(&terminal_metadata);
        let content_stop_state = Arc::new(Mutex::new(ContentStopState::default()));
        let token_stream_content_stop_state = Arc::clone(&content_stop_state);
        let effective_stop_for_tokens = effective_stop.clone();
        let trailing_stop_markers_for_tokens = trailing_stop_markers.clone();
        let trace_context_for_tokens = config.agent_trace.clone();
        let token_stream = backend_stream
            .then(move |chunk| {
                let completion_id = completion_id_for_tokens.clone();
                let model_name = model_name_for_tokens.clone();
                let error_flag = Arc::clone(&token_stream_error_flag);
                let completion_tokens = Arc::clone(&token_stream_completion_tokens);
                let terminal_metadata = Arc::clone(&token_stream_terminal_metadata);
                let content_stop_state = Arc::clone(&token_stream_content_stop_state);
                let effective_stop = effective_stop_for_tokens.clone();
                let trailing_stop_markers = trailing_stop_markers_for_tokens.clone();
                let trace_context = trace_context_for_tokens.clone();
                async move {
                    match chunk {
                        Ok(message) => {
                            if let Some(trace_context) = trace_context.as_ref() {
                                record_json_from_context(
                                    trace_context,
                                    "slab-app-core",
                                    "runtime_stream_chunk",
                                    runtime_chunk_payload(&message),
                                );
                            }
                            let decoded = text_chunk_from_runtime(message);
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
                                // Flush any held-back content from the stop
                                // state now that the stream is complete.
                                let emission = content_stop_state
                                    .lock()
                                    .expect("local content stop state lock poisoned")
                                    .finish(&effective_stop, &trailing_stop_markers);
                                if emission.matched {
                                    terminal.finish_reason = Some("stop".to_owned());
                                    if let Some(trace_context) = trace_context.as_ref() {
                                        record_json_from_context(
                                            trace_context,
                                            "slab-app-core",
                                            "local_stop_matched",
                                            serde_json::json!({
                                                "phase": "stream_finish_flush",
                                                "stop_sequences": effective_stop,
                                                "trailing_stop_markers": trailing_stop_markers,
                                            }),
                                        );
                                    }
                                }
                                let mut chunks = Vec::new();
                                if !emission.text.is_empty() {
                                    chunks.push(super::build_chunk(
                                        &completion_id,
                                        created_ts,
                                        &model_name,
                                        &emission.text,
                                    ));
                                }
                                chunks
                            } else if let Some(reasoning) =
                                reasoning_content_from_metadata(&decoded.metadata)
                            {
                                // The runtime layer has already separated
                                // reasoning from content via its own
                                // ThinkingStreamState. When reasoning is
                                // disabled for this request, suppress the
                                // reasoning side channel and fall back to the
                                // content delta, or the reasoning delta itself
                                // if the model never produced a visible answer.
                                let mut chunks = Vec::new();
                                let routed = route_stream_delta(
                                    &decoded.delta,
                                    Some(reasoning),
                                    reasoning_disabled,
                                );
                                if let Some(reasoning) = routed.reasoning.as_deref() {
                                    chunks.push(super::build_reasoning_chunk(
                                        &completion_id,
                                        created_ts,
                                        &model_name,
                                        reasoning,
                                    ));
                                }
                                if !routed.content.is_empty() {
                                    let emission = content_stop_state
                                        .lock()
                                        .expect("local content stop state lock poisoned")
                                        .ingest(
                                            &routed.content,
                                            &effective_stop,
                                            &trailing_stop_markers,
                                        );
                                    if emission.matched {
                                        terminal_metadata
                                            .lock()
                                            .expect("local chat terminal metadata lock poisoned")
                                            .finish_reason = Some("stop".to_owned());
                                        if let Some(trace_context) = trace_context.as_ref() {
                                            record_json_from_context(
                                                trace_context,
                                                "slab-app-core",
                                                "local_stop_matched",
                                                serde_json::json!({
                                                    "phase": "stream_reasoning_content",
                                                    "stop_sequences": effective_stop,
                                                    "trailing_stop_markers": trailing_stop_markers,
                                                }),
                                            );
                                        }
                                    }
                                    if !emission.text.is_empty() {
                                        chunks.push(super::build_chunk(
                                            &completion_id,
                                            created_ts,
                                            &model_name,
                                            &emission.text,
                                        ));
                                    }
                                }
                                chunks
                            } else if decoded.delta.is_empty() {
                                Vec::new()
                            } else {
                                // Plain content delta; the runtime has already
                                // stripped any <think> tags, so apply stop
                                // detection directly without re-parsing.
                                completion_tokens.fetch_add(1, Ordering::SeqCst);
                                let emission = content_stop_state
                                    .lock()
                                    .expect("local content stop state lock poisoned")
                                    .ingest(
                                        &decoded.delta,
                                        &effective_stop,
                                        &trailing_stop_markers,
                                    );
                                if emission.matched {
                                    terminal_metadata
                                        .lock()
                                        .expect("local chat terminal metadata lock poisoned")
                                        .finish_reason = Some("stop".to_owned());
                                    if let Some(trace_context) = trace_context.as_ref() {
                                        record_json_from_context(
                                            trace_context,
                                            "slab-app-core",
                                            "local_stop_matched",
                                            serde_json::json!({
                                                "phase": "stream_content",
                                                "stop_sequences": effective_stop,
                                                "trailing_stop_markers": trailing_stop_markers,
                                            }),
                                        );
                                    }
                                }
                                if !emission.text.is_empty() {
                                    vec![super::build_chunk(
                                        &completion_id,
                                        created_ts,
                                        &model_name,
                                        &emission.text,
                                    )]
                                } else {
                                    Vec::new()
                                }
                            }
                        }
                        Err(error) => {
                            error_flag.store(true, Ordering::SeqCst);
                            vec![super::build_error_chunk(&error.to_string())]
                        }
                    }
                }
            })
            .flat_map(stream::iter);

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
                Some(super::build_finish_chunk(
                    &completion_id_for_finish,
                    created_ts,
                    &model_name_for_finish,
                    &finish_reason,
                ))
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
                Some(super::build_usage_chunk(
                    &completion_id_for_usage,
                    created_ts,
                    &model_name_for_usage,
                    &usage,
                ))
            }
        })
        .filter_map(futures::future::ready);

        let sse_stream = role_chunk
            .chain(token_stream)
            .chain(finish_chunk)
            .chain(usage_chunk)
            .chain(stream::once(async { "[DONE]".to_owned() }))
            .map(move |item| {
                let _keep_alive = &usage_guard;
                item
            });

        return Ok(GeneratedChatOutput::Stream(Box::pin(sse_stream)));
    }

    let _usage_guard =
        state.auto_unload().acquire_for_inference(backend_id).await.map_err(|error| {
            AppCoreError::BackendNotReady(format!(
                "{} backend not ready: {error}",
                backend_id.canonical_id()
            ))
        })?;

    let runtime_response = state.runtime().chat(request).await?;
    if let Some(trace_context) = config.agent_trace.as_ref() {
        record_json_from_context(
            trace_context,
            "slab-app-core",
            "runtime_response",
            runtime_response_payload(&runtime_response),
        );
    }
    let mut response = text_response_from_runtime(runtime_response);

    let usage = response.usage.clone().unwrap_or_else(|| {
        super::build_estimated_usage(&prompt, &response.text, response.tokens_used)
    });
    response.tokens_used.get_or_insert(usage.completion_tokens);
    response.usage = Some(usage.clone());
    response.finish_reason.get_or_insert_with(|| {
        super::finish_reason_from_token_budget(usage.completion_tokens, config.max_tokens)
    });
    attach_reasoning_metadata(&mut response);
    if reasoning_is_disabled(config.reasoning_effort) {
        suppress_reasoning_output(&mut response);
    }
    let (trimmed_text, stop_matched) = super::apply_stop_sequences(&response.text, &effective_stop);
    if stop_matched {
        if let Some(trace_context) = config.agent_trace.as_ref() {
            record_json_from_context(
                trace_context,
                "slab-app-core",
                "local_stop_matched",
                serde_json::json!({
                    "phase": "text_response",
                    "stop_sequences": effective_stop,
                    "trailing_stop_markers": trailing_stop_markers,
                }),
            );
        }
        response.text = trimmed_text;
        response.finish_reason = Some("stop".to_owned());
    } else {
        let trimmed_text = trim_trailing_stop_markers(&response.text, &trailing_stop_markers);
        if trimmed_text.len() != response.text.len() {
            response.text = trimmed_text;
            response.finish_reason.get_or_insert_with(|| "stop".to_owned());
        }
    }

    Ok(GeneratedChatOutput::Text(response))
}

pub(super) async fn create_text_completion(
    state: &ModelState,
    model: &str,
    prompt: &str,
    config: LocalTextRequestConfig,
) -> Result<TextGenerationResponse, AppCoreError> {
    let prompt_profile = model::resolve_local_chat_prompt_profile(state, model).await?;
    let backend_id = prompt_profile.backend_id;
    let prompt =
        apply_local_reasoning_controls_to_prompt(prompt, config.reasoning_effort, config.verbosity);
    let gbnf = super::gbnf::resolve_effective_gbnf(
        config.gbnf.as_deref(),
        config.structured_output.as_ref(),
        prompt_profile.default_gbnf.as_deref(),
    )?;
    let request = RuntimeTextGenerationRequest {
        backend_id: Some(backend_id),
        model: model.to_owned(),
        prompt: prompt.clone(),
        system_prompt: None,
        max_tokens: Some(config.max_tokens),
        temperature: Some(config.temperature),
        top_p: config.top_p,
        top_k: config.top_k,
        min_p: config.min_p,
        presence_penalty: config.presence_penalty,
        repetition_penalty: config.repetition_penalty,
        session_key: None,
        stream: false,
        gbnf,
        stop_sequences: Vec::new(),
        agent_trace: None,
    };

    let _usage_guard =
        state.auto_unload().acquire_for_inference(backend_id).await.map_err(|error| {
            AppCoreError::BackendNotReady(format!(
                "{} backend not ready: {error}",
                backend_id.canonical_id()
            ))
        })?;

    let mut response = text_response_from_runtime(state.runtime().chat(request).await?);

    let usage = response.usage.clone().unwrap_or_else(|| {
        super::build_estimated_usage(&prompt, &response.text, response.tokens_used)
    });
    response.tokens_used.get_or_insert(usage.completion_tokens);
    response.usage = Some(usage.clone());
    response.finish_reason.get_or_insert_with(|| {
        super::finish_reason_from_token_budget(usage.completion_tokens, config.max_tokens)
    });

    Ok(response)
}

fn text_usage_from_runtime(usage: RuntimeTextGenerationUsage) -> TextGenerationUsage {
    TextGenerationUsage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        prompt_tokens_details: TextPromptTokensDetails {
            cached_tokens: usage.prompt_tokens_details.cached_tokens,
        },
        estimated: usage.estimated,
    }
}

fn runtime_request_payload(request: &RuntimeTextGenerationRequest) -> serde_json::Value {
    serde_json::json!({
        "model": request.model,
        "backend_id": request.backend_id.map(|backend| backend.canonical_id()),
        "prompt": request.prompt,
        "system_prompt": request.system_prompt,
        "max_tokens": request.max_tokens,
        "temperature": request.temperature,
        "top_p": request.top_p,
        "top_k": request.top_k,
        "min_p": request.min_p,
        "presence_penalty": request.presence_penalty,
        "repetition_penalty": request.repetition_penalty,
        "session_key": request.session_key,
        "stream": request.stream,
        "gbnf": request.gbnf,
        "stop_sequences": request.stop_sequences,
    })
}

fn runtime_response_payload(response: &RuntimeTextGenerationResponse) -> serde_json::Value {
    serde_json::json!({
        "text": response.text,
        "finish_reason": response.finish_reason,
        "tokens_used": response.tokens_used,
        "usage": response.usage.as_ref().map(runtime_usage_payload),
        "metadata": response.metadata,
    })
}

fn runtime_chunk_payload(chunk: &RuntimeTextGenerationChunk) -> serde_json::Value {
    serde_json::json!({
        "delta": chunk.delta,
        "done": chunk.done,
        "finish_reason": chunk.finish_reason,
        "usage": chunk.usage.as_ref().map(runtime_usage_payload),
        "metadata": chunk.metadata,
    })
}

fn runtime_usage_payload(usage: &RuntimeTextGenerationUsage) -> serde_json::Value {
    serde_json::json!({
        "prompt_tokens": usage.prompt_tokens,
        "completion_tokens": usage.completion_tokens,
        "total_tokens": usage.total_tokens,
        "prompt_tokens_details": {
            "cached_tokens": usage.prompt_tokens_details.cached_tokens,
        },
        "estimated": usage.estimated,
    })
}

fn text_response_from_runtime(response: RuntimeTextGenerationResponse) -> TextGenerationResponse {
    TextGenerationResponse {
        text: response.text,
        finish_reason: response.finish_reason,
        tokens_used: response.tokens_used,
        usage: response.usage.map(text_usage_from_runtime),
        metadata: response.metadata,
        tool_calls: Vec::new(),
    }
}

fn text_chunk_from_runtime(chunk: RuntimeTextGenerationChunk) -> TextGenerationChunk {
    TextGenerationChunk {
        delta: chunk.delta,
        done: chunk.done,
        finish_reason: chunk.finish_reason,
        usage: chunk.usage.map(text_usage_from_runtime),
        metadata: chunk.metadata,
    }
}
