use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use futures::{StreamExt, stream};
use slab_agent_tracing::record_json_from_context;
use slab_types::RuntimeBackendId;
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatReasoningEffort, ChatVerbosity, ConversationContentPart,
    ConversationMessage as DomainConversationMessage, ConversationMessageContent, StructuredOutput,
    TextGenerationResponse, TextGenerationUsage,
};
use crate::domain::ports::{RuntimeChatImagePart, RuntimeTextGenerationRequest};
use crate::domain::services::llm::local::{
    local_chat, local_chat_stream, runtime_chunk_payload, runtime_request_payload,
    runtime_response_payload, text_chunk_from_runtime, text_response_from_runtime,
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
pub(crate) struct LocalChatRequestConfig {
    pub(crate) session_id: Option<String>,
    pub(crate) max_tokens: u32,
    pub(crate) temperature: f32,
    pub(crate) top_p: Option<f32>,
    pub(crate) top_k: Option<i32>,
    pub(crate) min_p: Option<f32>,
    pub(crate) presence_penalty: Option<f32>,
    pub(crate) repetition_penalty: Option<f32>,
    pub(crate) reasoning_effort: Option<ChatReasoningEffort>,
    pub(crate) verbosity: Option<ChatVerbosity>,
    pub(crate) reasoning_guidance_in_context: bool,
    pub(crate) gbnf: Option<String>,
    pub(crate) structured_output: Option<StructuredOutput>,
    pub(crate) tools: Vec<slab_proto::openai::FunctionTool>,
    pub(crate) stop: Vec<String>,
    pub(crate) agent_trace: Option<slab_agent_tracing::AgentTraceContext>,
    pub(crate) stream: bool,
    pub(crate) include_usage: bool,
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

/// A resolved local runtime request plus the prompt-engineering byproducts the
/// chat streaming layer still needs (stop sequences + trailing markers for
/// output trimming / usage estimation). Shared between
/// [`create_chat_completion`] and the `/responses` single-shot path.
#[derive(Debug, Clone)]
pub(crate) struct LocalRuntimeRequest {
    pub(crate) backend_id: RuntimeBackendId,
    pub(crate) request: RuntimeTextGenerationRequest,
    pub(crate) prompt: String,
    pub(crate) effective_stop: Vec<String>,
    pub(crate) trailing_stop_markers: Vec<String>,
}

/// Stable sentinel substituted for each image part in the rendered prompt. The
/// runtime replaces every occurrence with the loaded projector's real media
/// marker before handing the prompt to `mtmd_tokenize` (which requires one
/// marker per image, in order).
pub(super) const MTMD_MEDIA_SENTINEL: &str = "<<SLAB_MTMD_MEDIA>>";

/// Decode an image `image_url` payload into raw encoded bytes (PNG/JPEG/…).
///
/// Handles OpenAI-style `data:<mediatype>;base64,<…>` data URIs (the common
/// local-inference case where the frontend embeds the image inline), bare
/// base64 strings, and local file paths. Remote `http(s)://` URLs return `None`
/// (local inference does not fetch them synchronously).
fn decode_image_url(url: &str) -> Option<(Vec<u8>, Option<String>)> {
    use base64::Engine as _;
    if let Some(rest) = url.strip_prefix("data:") {
        let comma = rest.find(',')?;
        let meta = &rest[..comma];
        let payload = &rest[comma + 1..];
        let mediatype = meta.split(';').next().filter(|s| s.contains('/')).map(str::to_owned);
        let bytes = if meta.contains("base64") {
            base64::engine::general_purpose::STANDARD.decode(payload).ok()?
        } else {
            payload.as_bytes().to_vec()
        };
        return Some((bytes, mediatype));
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        return None;
    }
    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(url) {
        return Some((bytes, None));
    }
    std::fs::read(url).ok().map(|bytes| (bytes, None))
}

/// Walk `messages` in place, replacing each `Image` content part with the
/// [`MTMD_MEDIA_SENTINEL`] text marker and collecting the decoded image bytes
/// (in order). Unresolvable images fall back to a `[image]` text placeholder and
/// contribute no bitmap, keeping the sentinel/bitmap counts aligned. Messages
/// without image parts are untouched — the text-only path is byte-identical to
/// before this call.
pub(super) fn extract_image_parts(
    messages: &mut [DomainConversationMessage],
) -> Vec<RuntimeChatImagePart> {
    let mut images = Vec::new();
    for message in messages.iter_mut() {
        let ConversationMessageContent::Parts(parts) = &mut message.content else {
            continue;
        };
        for part in parts.iter_mut() {
            if let ConversationContentPart::Image { image_url, mime_type, .. } = part {
                let resolved = image_url.as_deref().and_then(decode_image_url);
                match resolved {
                    Some((data, mime)) => {
                        images.push(RuntimeChatImagePart {
                            data,
                            mime_type: mime_type.clone().or(mime),
                        });
                        *part =
                            ConversationContentPart::Text { text: MTMD_MEDIA_SENTINEL.to_owned() };
                    }
                    None => {
                        *part = ConversationContentPart::Text { text: "[image]".to_owned() };
                    }
                }
            }
        }
    }
    images
}

/// Render the local chat prompt (template + reasoning controls + gbnf) and
/// assemble the [`RuntimeTextGenerationRequest`]. Shared by the chat streaming
/// layer ([`create_chat_completion`]) and the `/responses` single-shot path so
/// the local prompt engineering is not duplicated. Records the same agent-trace
/// payloads as the inline path did.
pub(crate) async fn build_local_runtime_request(
    state: &ModelState,
    model: &str,
    messages: &[DomainConversationMessage],
    config: &LocalChatRequestConfig,
) -> Result<LocalRuntimeRequest, AppCoreError> {
    let prompt_profile = model::resolve_local_chat_prompt_profile(state, model).await?;
    let backend_id = prompt_profile.backend_id;

    // Skip the inline local reasoning-policy injection when:
    //  - the Jinja chat template natively references `enable_thinking` (e.g.
    //    Qwen3, DeepSeek-R1) and already controls thinking via its template
    //    variable; or
    //  - the caller already injected a reasoning-effort fragment via the agent
    //    context hook (`reasoning_guidance_in_context`) — the agent path would
    //    otherwise be guided twice.
    let native_thinking =
        super::template::template_supports_thinking(prompt_profile.chat_template_source.as_deref());
    let skip_inline = native_thinking || config.reasoning_guidance_in_context;
    let injected_guidance = if skip_inline {
        None
    } else {
        local_reasoning_guidance(config.reasoning_effort, config.verbosity)
    };
    let mut request_messages = if skip_inline {
        messages.to_vec()
    } else {
        apply_local_reasoning_controls(messages, config.reasoning_effort, config.verbosity)
    };
    // Pull image parts out of the message content BEFORE the template flattens
    // them to text, substituting a sentinel marker at each image's position.
    // No-op (empty) for text-only turns.
    let image_parts = extract_image_parts(&mut request_messages);
    if let Some(trace_context) = config.agent_trace.as_ref() {
        record_json_from_context(
            trace_context,
            "slab-app-core",
            "local_reasoning_policy_injected",
            serde_json::json!({
                "native_thinking": native_thinking,
                "guidance_in_context": config.reasoning_guidance_in_context,
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
        image_parts,
    };
    if let Some(trace_context) = config.agent_trace.as_ref() {
        record_json_from_context(
            trace_context,
            "slab-app-core",
            "runtime_request",
            runtime_request_payload(&request),
        );
    }

    Ok(LocalRuntimeRequest { backend_id, request, prompt, effective_stop, trailing_stop_markers })
}

pub(super) async fn create_chat_completion(
    state: &ModelState,
    model: &str,
    messages: &[DomainConversationMessage],
    config: LocalChatRequestConfig,
) -> Result<GeneratedChatOutput, AppCoreError> {
    let LocalRuntimeRequest { backend_id, request, prompt, effective_stop, trailing_stop_markers } =
        build_local_runtime_request(state, model, messages, &config).await?;

    if config.stream {
        let (backend_stream, usage_guard) = local_chat_stream(state, backend_id, request).await?;

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

    let runtime_response = local_chat(state, backend_id, request).await?;
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
        image_parts: Vec::new(),
    };

    let mut response = text_response_from_runtime(local_chat(state, backend_id, request).await?);

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

// runtime exec + RuntimeTextGeneration* -> TextGeneration* conversion + trace payloads
// have been pushed down to `domain::services::llm::local`, shared by chat and the future
// response service.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::{ConversationContentPart, ConversationMessageContent};

    fn text_msg(role: &str, text: &str) -> DomainConversationMessage {
        DomainConversationMessage {
            role: role.to_owned(),
            content: ConversationMessageContent::Text(text.to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    #[test]
    fn extract_image_parts_replaces_image_with_sentinel() {
        // 1x1 PNG as a base64 data URI (the common local-inference encoding).
        let data_uri = format!(
            "data:image/png;base64,{}",
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
        );
        let mut messages = vec![
            text_msg("user", "hello"),
            DomainConversationMessage {
                role: "user".to_owned(),
                content: ConversationMessageContent::Parts(vec![
                    ConversationContentPart::Text { text: "what is this? ".to_owned() },
                    ConversationContentPart::Image {
                        image_url: Some(data_uri),
                        mime_type: None,
                        detail: None,
                    },
                ]),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
        ];
        let images = extract_image_parts(&mut messages);
        assert_eq!(images.len(), 1);
        assert!(!images[0].data.is_empty());
        assert_eq!(images[0].mime_type.as_deref(), Some("image/png"));
        // The image part is replaced by the sentinel marker; text part untouched.
        let ConversationMessageContent::Parts(parts) = &messages[1].content else {
            panic!("content should still be Parts");
        };
        assert_eq!(parts.len(), 2);
        let ConversationContentPart::Text { text } = &parts[1] else {
            panic!("image part should have been replaced by a Text sentinel");
        };
        assert_eq!(text, MTMD_MEDIA_SENTINEL);
    }

    #[test]
    fn extract_image_parts_is_noop_for_text_only() {
        let mut messages = vec![text_msg("user", "plain text"), text_msg("assistant", "reply")];
        let images = extract_image_parts(&mut messages);
        assert!(images.is_empty());
        assert_eq!(messages[0].content, ConversationMessageContent::Text("plain text".to_owned()));
        assert_eq!(messages[1].content, ConversationMessageContent::Text("reply".to_owned()));
    }

    #[test]
    fn extract_image_parts_drops_unresolvable_http_url() {
        let mut messages = vec![DomainConversationMessage {
            role: "user".to_owned(),
            content: ConversationMessageContent::Parts(vec![ConversationContentPart::Image {
                image_url: Some("https://example.com/cat.png".to_owned()),
                mime_type: None,
                detail: None,
            }]),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }];
        let images = extract_image_parts(&mut messages);
        // Remote URLs are not fetched for local inference → no bitmap, fallback text.
        assert!(images.is_empty());
    }
}
