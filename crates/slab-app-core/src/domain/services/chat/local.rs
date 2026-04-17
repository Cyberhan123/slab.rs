use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use futures::{StreamExt, stream};
use serde_json::Value;
use slab_proto::convert;
use slab_types::inference::{TextGenerationRequest, TextGenerationResponse, TextGenerationUsage};
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatReasoningEffort, ChatStreamChunk, ChatVerbosity,
    ConversationMessage as DomainConversationMessage, ConversationMessageContent, StructuredOutput,
};
use crate::domain::services::model;
use crate::error::AppCoreError;
use crate::infra::rpc;

use super::GeneratedChatOutput;

const REASONING_CONTENT_METADATA_KEY: &str = "reasoning_content";
const THINK_OPEN_MARKER: &str = "<think";
const THINK_CLOSE_TAG: &str = "</think>";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedThinkingOutput {
    content: String,
    reasoning: String,
}



#[derive(Debug, Default)]
struct ContentStopState {
    raw_content: String,
    emitted_len: usize,
    matched: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct StopEmission {
    text: String,
    matched: bool,
}

fn trailing_partial_marker_len(raw: &str, marker: &str) -> usize {
    let max = raw.len().min(marker.len().saturating_sub(1));
    (1..=max).rev().find(|len| raw.ends_with(&marker[..*len])).unwrap_or(0)
}

fn parse_thinking_output(raw: &str, complete: bool) -> ParsedThinkingOutput {
    let Some(open_start) = raw.find(THINK_OPEN_MARKER) else {
        // No <think found — treat all text as content.
        return ParsedThinkingOutput {
            content: raw.to_owned(),
            reasoning: String::new(),
        };
    };

    let content_prefix = raw[..open_start].to_owned();
    let after_open_marker = &raw[open_start..];
    let Some(open_end_rel) = after_open_marker.find('>') else {
        return ParsedThinkingOutput {
            content: if complete { raw.to_owned() } else { content_prefix },
            reasoning: String::new(),
        };
    };

    let reasoning_start = open_start + open_end_rel + 1;
    let after_open = &raw[reasoning_start..];
    if let Some(close_rel) = after_open.find(THINK_CLOSE_TAG) {
        let close_start = reasoning_start + close_rel;
        let close_end = close_start + THINK_CLOSE_TAG.len();
        let mut content = content_prefix;
        content.push_str(&raw[close_end..]);
        return ParsedThinkingOutput {
            content,
            reasoning: raw[reasoning_start..close_start].to_owned(),
        };
    }

    let stable_reasoning_end = if complete {
        raw.len()
    } else {
        raw.len().saturating_sub(trailing_partial_marker_len(raw, THINK_CLOSE_TAG))
    };
    ParsedThinkingOutput {
        content: content_prefix,
        reasoning: raw[reasoning_start..stable_reasoning_end].to_owned(),
    }
}

fn reasoning_content_from_metadata(metadata: &slab_types::inference::JsonOptions) -> Option<&str> {
    metadata.get(REASONING_CONTENT_METADATA_KEY).and_then(Value::as_str)
}

fn trailing_partial_stop_len(raw: &str, stop: &[String]) -> usize {
    stop.iter()
        .filter(|value| value.len() > 1)
        .map(|value| {
            let max = raw.len().min(value.len().saturating_sub(1));
            (1..=max)
                .rev()
                .find(|len| raw.ends_with(&value[..*len]))
                .unwrap_or(0)
        })
        .max()
        .unwrap_or(0)
}

fn stable_stop_boundary(raw: &str, stop: &[String], complete: bool) -> (usize, bool) {
    if let Some((index, _)) = stop
        .iter()
        .filter(|value| !value.is_empty())
        .filter_map(|value| raw.find(value).map(|index| (index, value)))
        .min_by_key(|(index, _)| *index)
    {
        return (index, true);
    }

    if complete {
        return (raw.len(), false);
    }

    let hold_len = trailing_partial_stop_len(raw, stop);
    (raw.len().saturating_sub(hold_len), false)
}

fn trailing_trim_len(raw: &str, trailing: &[String]) -> usize {
    trailing
        .iter()
        .filter(|value| !value.is_empty())
        .filter(|value| raw.ends_with(value.as_str()))
        .map(|value| value.len())
        .max()
        .unwrap_or(0)
}

fn trim_trailing_stop_markers(raw: &str, trailing: &[String]) -> String {
    let trim_len = trailing_trim_len(raw, trailing);
    if trim_len == 0 {
        raw.to_owned()
    } else {
        raw[..raw.len().saturating_sub(trim_len)].to_owned()
    }
}

fn merge_stop_sequences(primary: &[String], extra: &[String]) -> Vec<String> {
    let mut merged = Vec::new();
    for value in primary.iter().chain(extra.iter()) {
        if value.is_empty() || merged.iter().any(|existing| existing == value) {
            continue;
        }
        merged.push(value.clone());
    }
    merged
}



impl ContentStopState {
    fn ingest(&mut self, delta: &str, stop: &[String], trailing: &[String]) -> StopEmission {
        if self.matched || delta.is_empty() {
            return StopEmission::default();
        }

        self.raw_content.push_str(delta);
        self.emit(stop, trailing, false)
    }

    fn finish(&mut self, stop: &[String], trailing: &[String]) -> StopEmission {
        self.emit(stop, trailing, true)
    }

    fn emit(&mut self, stop: &[String], trailing: &[String], complete: bool) -> StopEmission {
        if self.matched {
            return StopEmission::default();
        }

        let (mut visible_len, matched) = stable_stop_boundary(&self.raw_content, stop, complete);
        if complete && !matched {
            let trim_len = trailing_trim_len(&self.raw_content[..visible_len], trailing);
            visible_len = visible_len.saturating_sub(trim_len);
        }

        let text = if visible_len > self.emitted_len {
            self.raw_content[self.emitted_len..visible_len].to_owned()
        } else {
            String::new()
        };
        self.emitted_len = visible_len;
        if matched {
            self.matched = true;
        }

        StopEmission { text, matched }
    }
}

fn attach_reasoning_metadata(response: &mut TextGenerationResponse) {
    if reasoning_content_from_metadata(&response.metadata).is_some() {
        return;
    }

    let parsed = parse_thinking_output(&response.text, true);
    let reasoning = parsed.reasoning.trim();
    if reasoning.is_empty() {
        return;
    }

    response.text = parsed.content;
    response
        .metadata
        .insert(REASONING_CONTENT_METADATA_KEY.into(), Value::String(reasoning.to_owned()));
}

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
    pub(super) stop: Vec<String>,
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

fn local_reasoning_guidance(
    reasoning_effort: Option<ChatReasoningEffort>,
    verbosity: Option<ChatVerbosity>,
) -> Option<String> {
    let mut lines = Vec::new();

    match reasoning_effort {
        Some(ChatReasoningEffort::None) => {
            lines.push(
                "Answer directly and do not emit <think>...</think> blocks or hidden reasoning."
                    .to_owned(),
            );
        }
        Some(ChatReasoningEffort::Minimal) => {
            lines.push(
                "If reasoning is necessary, keep it minimal and place it in a very short <think>...</think> block before the final answer."
                    .to_owned(),
            );
        }
        Some(ChatReasoningEffort::Low) => {
            lines.push(
                "If reasoning is useful, keep the <think>...</think> block brief and keep the final answer outside the block."
                    .to_owned(),
            );
        }
        Some(ChatReasoningEffort::Medium) => {
            lines.push(
                "If reasoning is useful, use a moderate <think>...</think> block and keep the final answer outside the block."
                    .to_owned(),
            );
        }
        Some(ChatReasoningEffort::High) => {
            lines.push(
                "You may use a detailed <think>...</think> block before the final answer, but keep the final answer outside the block."
                    .to_owned(),
            );
        }
        None => {}
    }

    match verbosity {
        Some(ChatVerbosity::Low) => {
            lines.push("Keep the final answer concise and compact.".to_owned());
        }
        Some(ChatVerbosity::Medium) => {
            lines.push("Keep the final answer moderately detailed.".to_owned());
        }
        Some(ChatVerbosity::High) => {
            lines.push("Keep the final answer detailed and thorough.".to_owned());
        }
        None => {}
    }

    if lines.is_empty() {
        None
    } else {
        Some(format!("Local response policy:\n{}", lines.join("\n")))
    }
}

fn apply_local_reasoning_controls(
    messages: &[DomainConversationMessage],
    reasoning_effort: Option<ChatReasoningEffort>,
    verbosity: Option<ChatVerbosity>,
) -> Vec<DomainConversationMessage> {
    let Some(guidance) = local_reasoning_guidance(reasoning_effort, verbosity) else {
        return messages.to_vec();
    };

    let insert_at = messages
        .iter()
        .take_while(|message| matches!(message.role.as_str(), "system" | "developer"))
        .count();
    let mut guided_messages = messages.to_vec();
    guided_messages.insert(
        insert_at,
        DomainConversationMessage {
            role: "system".to_owned(),
            content: ConversationMessageContent::Text(guidance),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        },
    );
    guided_messages
}

fn apply_local_reasoning_controls_to_prompt(
    prompt: &str,
    reasoning_effort: Option<ChatReasoningEffort>,
    verbosity: Option<ChatVerbosity>,
) -> String {
    match local_reasoning_guidance(reasoning_effort, verbosity) {
        Some(guidance) => format!("{guidance}\n\nPrompt:\n{prompt}"),
        None => prompt.to_owned(),
    }
}

pub(super) async fn create_chat_completion(
    state: &ModelState,
    model: &str,
    messages: &[DomainConversationMessage],
    config: LocalChatRequestConfig,
) -> Result<GeneratedChatOutput, AppCoreError> {
    let prompt_profile = model::resolve_local_chat_prompt_profile(state, model).await?;

    // When the Jinja chat template natively references `enable_thinking` (e.g.
    // Qwen3, DeepSeek-R1), it already controls thinking behaviour via the
    // template variable.  Skip injecting an extra system-level reasoning
    // guidance message — it can confuse smaller models and conflict with the
    // template's own thinking protocol.
    let native_thinking =
        super::template::template_supports_thinking(prompt_profile.chat_template_source.as_deref());
    let request_messages = if native_thinking {
        messages.to_vec()
    } else {
        apply_local_reasoning_controls(messages, config.reasoning_effort, config.verbosity)
    };

    let prompt = super::template::build_prompt(
        &request_messages,
        prompt_profile.chat_template_source.as_deref(),
        config.reasoning_effort,
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
    let request = TextGenerationRequest {
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
        ..Default::default()
    };
    let grpc_request = convert::encode_chat_request(model.to_owned(), &request);

    let llama_channel = state.grpc().chat_channel().ok_or_else(|| {
        AppCoreError::BackendNotReady("llama gRPC endpoint is not configured".into())
    })?;

    if config.stream {
        let usage_guard =
            state.auto_unload().acquire_for_inference(super::LLAMA_BACKEND_ID).await.map_err(
                |error| AppCoreError::BackendNotReady(format!("llama backend not ready: {error}")),
            )?;

        let backend_stream = rpc::client::chat_stream(llama_channel.clone(), grpc_request.clone())
            .await
            .map_err(map_runtime_chat_error("chat stream"))?;

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
        let content_stop_state = Arc::new(Mutex::new(ContentStopState::default()));
        let token_stream_content_stop_state = Arc::clone(&content_stop_state);
        let effective_stop_for_tokens = effective_stop.clone();
        let trailing_stop_markers_for_tokens = trailing_stop_markers.clone();
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
                async move {
                    match chunk {
                        Ok(message) if !message.error.is_empty() => {
                            error_flag.store(true, Ordering::SeqCst);
                            vec![ChatStreamChunk::Data(super::build_error_chunk(&message.error))]
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
                                // Flush any held-back content from the stop
                                // state now that the stream is complete.
                                let emission = content_stop_state
                                    .lock()
                                    .expect("local content stop state lock poisoned")
                                    .finish(&effective_stop, &trailing_stop_markers);
                                if emission.matched {
                                    terminal.finish_reason = Some("stop".to_owned());
                                }
                                let mut chunks = Vec::new();
                                if !emission.text.is_empty() {
                                    chunks.push(ChatStreamChunk::Data(super::build_chunk(
                                        &completion_id,
                                        created_ts,
                                        &model_name,
                                        &emission.text,
                                    )));
                                }
                                chunks
                            } else if let Some(reasoning) =
                                reasoning_content_from_metadata(&decoded.metadata)
                            {
                                // The runtime layer has already separated
                                // reasoning from content via its own
                                // ThinkingStreamState. Pass reasoning through
                                // directly and apply stop detection to content.
                                let mut chunks = Vec::new();
                                if !reasoning.is_empty() {
                                    chunks.push(ChatStreamChunk::Data(
                                        super::build_reasoning_chunk(
                                            &completion_id,
                                            created_ts,
                                            &model_name,
                                            reasoning,
                                        ),
                                    ));
                                }
                                if !decoded.delta.is_empty() {
                                    let emission = content_stop_state
                                        .lock()
                                        .expect("local content stop state lock poisoned")
                                        .ingest(&decoded.delta, &effective_stop, &trailing_stop_markers);
                                    if emission.matched {
                                        terminal_metadata
                                            .lock()
                                            .expect("local chat terminal metadata lock poisoned")
                                            .finish_reason = Some("stop".to_owned());
                                    }
                                    if !emission.text.is_empty() {
                                        chunks.push(ChatStreamChunk::Data(super::build_chunk(
                                            &completion_id,
                                            created_ts,
                                            &model_name,
                                            &emission.text,
                                        )));
                                    }
                                }
                                chunks
                            } else if decoded.delta.is_empty() {
                                Vec::new()
                            } else {
                                // Plain content delta — the runtime has already
                                // stripped any <think> tags, so apply stop
                                // detection directly without re-parsing.
                                completion_tokens.fetch_add(1, Ordering::SeqCst);
                                let emission = content_stop_state
                                    .lock()
                                    .expect("local content stop state lock poisoned")
                                    .ingest(&decoded.delta, &effective_stop, &trailing_stop_markers);
                                if emission.matched {
                                    terminal_metadata
                                        .lock()
                                        .expect("local chat terminal metadata lock poisoned")
                                        .finish_reason = Some("stop".to_owned());
                                }
                                if !emission.text.is_empty() {
                                    vec![ChatStreamChunk::Data(super::build_chunk(
                                        &completion_id,
                                        created_ts,
                                        &model_name,
                                        &emission.text,
                                    ))]
                                } else {
                                    Vec::new()
                                }
                            }
                        }
                        Err(error) => {
                            error_flag.store(true, Ordering::SeqCst);
                            vec![ChatStreamChunk::Data(super::build_error_chunk(
                                &error.to_string(),
                            ))]
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
            |error| AppCoreError::BackendNotReady(format!("llama backend not ready: {error}")),
        )?;

    let generated = rpc::client::chat(llama_channel, grpc_request)
        .await
        .map_err(map_runtime_chat_error("chat"))?;
    let mut response = convert::decode_chat_response(&generated);

    let usage = response.usage.clone().unwrap_or_else(|| {
        super::build_estimated_usage(&prompt, &response.text, response.tokens_used)
    });
    response.tokens_used.get_or_insert(usage.completion_tokens);
    response.usage = Some(usage.clone());
    response.finish_reason.get_or_insert_with(|| {
        super::finish_reason_from_token_budget(usage.completion_tokens, config.max_tokens)
    });
    attach_reasoning_metadata(&mut response);
    let (trimmed_text, stop_matched) = super::apply_stop_sequences(&response.text, &effective_stop);
    if stop_matched {
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
) -> Result<slab_types::inference::TextGenerationResponse, AppCoreError> {
    let prompt_profile = model::resolve_local_chat_prompt_profile(state, model).await?;
    let prompt =
        apply_local_reasoning_controls_to_prompt(prompt, config.reasoning_effort, config.verbosity);
    let gbnf = super::gbnf::resolve_effective_gbnf(
        config.gbnf.as_deref(),
        config.structured_output.as_ref(),
        prompt_profile.default_gbnf.as_deref(),
    )?;
    let request = TextGenerationRequest {
        prompt: prompt.clone(),
        system_prompt: None,
        max_tokens: Some(config.max_tokens),
        temperature: Some(config.temperature),
        top_p: config.top_p,
        top_k: config.top_k,
        min_p: config.min_p,
        presence_penalty: config.presence_penalty,
        repetition_penalty: config.repetition_penalty,
        stream: false,
        gbnf,
        ..Default::default()
    };
    let grpc_request = convert::encode_chat_request(model.to_owned(), &request);

    let llama_channel = state.grpc().chat_channel().ok_or_else(|| {
        AppCoreError::BackendNotReady("llama gRPC endpoint is not configured".into())
    })?;

    let _usage_guard =
        state.auto_unload().acquire_for_inference(super::LLAMA_BACKEND_ID).await.map_err(
            |error| AppCoreError::BackendNotReady(format!("llama backend not ready: {error}")),
        )?;

    let generated = rpc::client::chat(llama_channel, grpc_request)
        .await
        .map_err(map_runtime_chat_error("chat"))?;
    let mut response = convert::decode_chat_response(&generated);

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

fn map_runtime_chat_error(
    action: &'static str,
) -> impl Fn(anyhow::Error) -> AppCoreError + Send + Sync + 'static {
    move |error| {
        if let Some(detail) = rpc::client::transient_runtime_detail(&error) {
            AppCoreError::BackendNotReady(detail)
        } else {
            AppCoreError::Internal(format!("grpc {action} failed: {error}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ParsedThinkingOutput, apply_local_reasoning_controls,
        apply_local_reasoning_controls_to_prompt, attach_reasoning_metadata,
        local_reasoning_guidance, parse_thinking_output, reasoning_content_from_metadata,
        trim_trailing_stop_markers,
    };
    use crate::domain::models::{
        ChatReasoningEffort, ChatVerbosity, ConversationMessage as DomainConversationMessage,
        ConversationMessageContent,
    };
    use serde_json::json;
    use slab_types::inference::TextGenerationResponse;

    #[test]
    fn parse_thinking_output_extracts_reasoning_block() {
        let parsed = parse_thinking_output("<think>step one</think>\n\nfinal answer", true);
        assert_eq!(
            parsed,
            ParsedThinkingOutput {
                content: "\n\nfinal answer".to_owned(),
                reasoning: "step one".to_owned(),
            }
        );
    }

    #[test]
    fn parse_thinking_output_no_think_tag_passes_through() {
        // Without <think>, all text is content — no hold-back.
        let parsed = parse_thinking_output("answer<th", false);
        assert_eq!(
            parsed,
            ParsedThinkingOutput { content: "answer<th".to_owned(), reasoning: String::new() }
        );
    }

    #[test]
    fn attach_reasoning_metadata_moves_reasoning_out_of_text() {
        let mut response = TextGenerationResponse {
            text: "<think>step by step</think>\n\nanswer".to_owned(),
            metadata: Default::default(),
            ..Default::default()
        };

        attach_reasoning_metadata(&mut response);

        assert_eq!(response.text, "\n\nanswer");
        assert_eq!(response.metadata.get("reasoning_content"), Some(&json!("step by step")));
    }

    #[test]
    fn attach_reasoning_metadata_keeps_runtime_reasoning_metadata() {
        let mut response = TextGenerationResponse {
            text: "answer".to_owned(),
            metadata: [("reasoning_content".to_owned(), json!("from runtime"))]
                .into_iter()
                .collect(),
            ..Default::default()
        };

        attach_reasoning_metadata(&mut response);

        assert_eq!(response.text, "answer");
        assert_eq!(reasoning_content_from_metadata(&response.metadata), Some("from runtime"));
    }

    #[test]
    fn local_reasoning_guidance_disables_think_blocks() {
        let guidance = local_reasoning_guidance(Some(ChatReasoningEffort::None), None)
            .expect("guidance should be produced");

        assert!(guidance.contains("do not emit <think>...</think>"));
    }

    #[test]
    fn apply_local_reasoning_controls_inserts_system_policy_after_existing_system_messages() {
        let messages = vec![
            DomainConversationMessage {
                role: "system".to_owned(),
                content: ConversationMessageContent::Text("existing".to_owned()),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
            DomainConversationMessage {
                role: "user".to_owned(),
                content: ConversationMessageContent::Text("hello".to_owned()),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
        ];

        let guided = apply_local_reasoning_controls(
            &messages,
            Some(ChatReasoningEffort::Low),
            Some(ChatVerbosity::High),
        );

        assert_eq!(guided.len(), 3);
        assert_eq!(guided[0].role, "system");
        assert_eq!(guided[1].role, "system");
        assert!(guided[1].rendered_text().contains("<think>...</think> block"));
        assert_eq!(guided[2].role, "user");
    }

    #[test]
    fn apply_local_reasoning_controls_to_prompt_prefixes_guidance() {
        let prompt = apply_local_reasoning_controls_to_prompt(
            "Solve 2+2",
            Some(ChatReasoningEffort::Minimal),
            Some(ChatVerbosity::Low),
        );

        assert!(prompt.starts_with("Local response policy:"));
        assert!(prompt.contains("Prompt:\nSolve 2+2"));
    }

    #[test]
    fn content_stop_state_trims_chatml_boundaries() {
        let stop = vec!["<|im_end|>".to_owned(), "<|endoftext|><|im_start|>".to_owned()];
        let trailing = vec!["<|endoftext|>".to_owned()];
        let mut state = super::ContentStopState::default();

        let first = state.ingest("hello<|endoftext|>", &stop, &trailing);
        let last = state.finish(&stop, &trailing);

        assert_eq!(
            first,
            super::StopEmission {
                text: "hello".to_owned(),
                matched: false,
            }
        );
        assert_eq!(last, super::StopEmission::default());
    }

    #[test]
    fn content_stop_state_stops_before_im_end_marker() {
        let stop = vec!["<|im_end|>".to_owned()];
        let trailing = Vec::new();
        let mut state = super::ContentStopState::default();

        let first = state.ingest("hello<|im", &stop, &trailing);
        let second = state.ingest("_end|>ignored", &stop, &trailing);

        assert_eq!(
            first,
            super::StopEmission {
                text: "hello".to_owned(),
                matched: false,
            }
        );
        assert_eq!(
            second,
            super::StopEmission {
                text: String::new(),
                matched: true,
            }
        );
        assert!(state.finish(&stop, &trailing).text.is_empty());
    }

    #[test]
    fn content_stop_state_stops_before_raw_chat_role_marker() {
        let stop = vec!["\nUser:".to_owned(), "\nAssistant:".to_owned()];
        let trailing = Vec::new();
        let mut state = super::ContentStopState::default();

        let first = state.ingest("hello\nUs", &stop, &trailing);
        let second = state.ingest("er: next turn", &stop, &trailing);

        assert_eq!(
            first,
            super::StopEmission {
                text: "hello".to_owned(),
                matched: false,
            }
        );
        assert_eq!(
            second,
            super::StopEmission {
                text: String::new(),
                matched: true,
            }
        );
        assert!(state.finish(&stop, &trailing).text.is_empty());
    }

    #[test]
    fn trim_trailing_stop_markers_removes_final_endoftext() {
        let trimmed =
            trim_trailing_stop_markers("answer<|endoftext|>", &["<|endoftext|>".to_owned()]);

        assert_eq!(trimmed, "answer");
    }
}
