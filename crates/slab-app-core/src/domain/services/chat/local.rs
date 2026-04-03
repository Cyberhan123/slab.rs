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
    ChatStreamChunk, ConversationMessage as DomainConversationMessage, UnifiedModel,
};
use crate::error::AppCoreError;
use crate::infra::db::ModelStore;
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum ThinkingDelta {
    Content(String),
    Reasoning(String),
}

#[derive(Debug, Default)]
struct ThinkingStreamState {
    raw_output: String,
    emitted_content_len: usize,
    emitted_reasoning_len: usize,
}

fn trailing_partial_marker_len(raw: &str, marker: &str) -> usize {
    let max = raw.len().min(marker.len().saturating_sub(1));
    (1..=max).rev().find(|len| raw.ends_with(&marker[..*len])).unwrap_or(0)
}

fn parse_thinking_output(raw: &str, complete: bool) -> ParsedThinkingOutput {
    let Some(open_start) = raw.find(THINK_OPEN_MARKER) else {
        let stable_end = if complete {
            raw.len()
        } else {
            raw.len().saturating_sub(trailing_partial_marker_len(raw, THINK_OPEN_MARKER))
        };
        return ParsedThinkingOutput {
            content: raw[..stable_end].to_owned(),
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

impl ThinkingStreamState {
    fn ingest(&mut self, delta: &str) -> Vec<ThinkingDelta> {
        self.raw_output.push_str(delta);
        self.emit(false)
    }

    fn finish(&mut self) -> Vec<ThinkingDelta> {
        self.emit(true)
    }

    fn emit(&mut self, complete: bool) -> Vec<ThinkingDelta> {
        let parsed = parse_thinking_output(&self.raw_output, complete);
        let mut deltas = Vec::new();

        if parsed.reasoning.len() > self.emitted_reasoning_len {
            deltas.push(ThinkingDelta::Reasoning(
                parsed.reasoning[self.emitted_reasoning_len..].to_owned(),
            ));
            self.emitted_reasoning_len = parsed.reasoning.len();
        }

        if parsed.content.len() > self.emitted_content_len {
            deltas.push(ThinkingDelta::Content(
                parsed.content[self.emitted_content_len..].to_owned(),
            ));
            self.emitted_content_len = parsed.content.len();
        }

        deltas
    }
}

fn attach_reasoning_metadata(response: &mut TextGenerationResponse) {
    let parsed = parse_thinking_output(&response.text, true);
    let reasoning = parsed.reasoning.trim();
    if reasoning.is_empty() {
        return;
    }

    response.text = parsed.content;
    response.metadata.insert(
        REASONING_CONTENT_METADATA_KEY.into(),
        Value::String(reasoning.to_owned()),
    );
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
) -> Result<GeneratedChatOutput, AppCoreError> {
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
        let thinking_state = Arc::new(Mutex::new(ThinkingStreamState::default()));
        let token_stream_thinking_state = Arc::clone(&thinking_state);
        let token_stream = backend_stream.then(move |chunk| {
            let completion_id = completion_id_for_tokens.clone();
            let model_name = model_name_for_tokens.clone();
            let error_flag = Arc::clone(&token_stream_error_flag);
            let completion_tokens = Arc::clone(&token_stream_completion_tokens);
            let terminal_metadata = Arc::clone(&token_stream_terminal_metadata);
            let thinking_state = Arc::clone(&token_stream_thinking_state);
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
                            thinking_state
                                .lock()
                                .expect("local thinking state lock poisoned")
                                .finish()
                                .into_iter()
                                .filter_map(|delta| match delta {
                                    ThinkingDelta::Reasoning(token) if !token.is_empty() => {
                                        Some(ChatStreamChunk::Data(super::build_reasoning_chunk(
                                            &completion_id,
                                            created_ts,
                                            &model_name,
                                            &token,
                                        )))
                                    }
                                    ThinkingDelta::Content(token) if !token.is_empty() => {
                                        Some(ChatStreamChunk::Data(super::build_chunk(
                                            &completion_id,
                                            created_ts,
                                            &model_name,
                                            &token,
                                        )))
                                    }
                                    _ => None,
                                })
                                .collect()
                        } else if decoded.delta.is_empty() {
                            Vec::new()
                        } else {
                            completion_tokens.fetch_add(1, Ordering::SeqCst);
                            thinking_state
                                .lock()
                                .expect("local thinking state lock poisoned")
                                .ingest(&decoded.delta)
                                .into_iter()
                                .filter_map(|delta| match delta {
                                    ThinkingDelta::Reasoning(token) if !token.is_empty() => {
                                        Some(ChatStreamChunk::Data(super::build_reasoning_chunk(
                                            &completion_id,
                                            created_ts,
                                            &model_name,
                                            &token,
                                        )))
                                    }
                                    ThinkingDelta::Content(token) if !token.is_empty() => {
                                        Some(ChatStreamChunk::Data(super::build_chunk(
                                            &completion_id,
                                            created_ts,
                                            &model_name,
                                            &token,
                                        )))
                                    }
                                    _ => None,
                                })
                                .collect()
                        }
                    }
                    Err(error) => {
                        error_flag.store(true, Ordering::SeqCst);
                        vec![ChatStreamChunk::Data(super::build_error_chunk(&error.to_string()))]
                    }
                }
            }
        }).flat_map(stream::iter);

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

    Ok(GeneratedChatOutput::Text(response))
}

pub(super) async fn create_text_completion(
    state: &ModelState,
    model: &str,
    prompt: &str,
    config: LocalTextRequestConfig,
) -> Result<slab_types::inference::TextGenerationResponse, AppCoreError> {
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
) -> Result<Option<super::template::PromptTemplateContext>, AppCoreError> {
    let Some(record) = state.store().get_model(model).await? else {
        return Ok(None);
    };
    let model: UnifiedModel =
        record.try_into().map_err(|error: String| AppCoreError::Internal(error))?;
    Ok(Some(super::template::PromptTemplateContext::from_model(&model)))
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
        ParsedThinkingOutput, ThinkingDelta, ThinkingStreamState, attach_reasoning_metadata,
        parse_thinking_output,
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
    fn parse_thinking_output_holds_partial_open_tag_until_complete() {
        let parsed = parse_thinking_output("answer<th", false);
        assert_eq!(
            parsed,
            ParsedThinkingOutput { content: "answer".to_owned(), reasoning: String::new() }
        );

        let completed = parse_thinking_output("answer<th", true);
        assert_eq!(
            completed,
            ParsedThinkingOutput { content: "answer<th".to_owned(), reasoning: String::new() }
        );
    }

    #[test]
    fn thinking_stream_state_splits_reasoning_and_content_deltas() {
        let mut state = ThinkingStreamState::default();
        assert!(state.ingest("<th").is_empty());
        assert_eq!(state.ingest("ink>first thought"), vec![ThinkingDelta::Reasoning("first thought".to_owned())]);
        assert_eq!(
            state.ingest("</think>\n\nfinal answer"),
            vec![ThinkingDelta::Content("\n\nfinal answer".to_owned())]
        );
        assert!(state.finish().is_empty());
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
        assert_eq!(
            response.metadata.get("reasoning_content"),
            Some(&json!("step by step"))
        );
    }
}
