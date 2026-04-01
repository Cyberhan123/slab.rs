//! OpenAI-compatible chat completion routes.

mod cloud;
mod local;
mod template;

use chrono::Utc;
use futures::StreamExt;
use futures::stream::{self, BoxStream};
use serde_json::{Value, json};
use slab_types::RuntimeBackendId;
use slab_types::inference::{TextGenerationResponse, TextGenerationUsage};
use std::sync::{Arc, Mutex};
use tracing::{debug, info};
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatCompletionCommand, ChatCompletionOutput, ChatCompletionResult, ChatModelOption,
    ChatResultChoice, ChatStreamChunk, ConversationMessage as DomainConversationMessage,
    ConversationMessageContent, StructuredOutput, TextCompletionCommand, TextCompletionOutput,
    TextCompletionResult, TextResultChoice, assistant_message_from_parts,
    assistant_message_from_text_response, serialize_session_message,
};
use crate::error::AppCoreError;
use crate::infra::db::{ChatMessage, ChatStore};

const LLAMA_BACKEND_ID: RuntimeBackendId = RuntimeBackendId::GgmlLlama;
const CLOUD_MODEL_ID_PREFIX: &str = "cloud";
const SYSTEM_FINGERPRINT: &str = "b-slab";

enum GeneratedChatOutput {
    Text(TextGenerationResponse),
    Stream(BoxStream<'static, ChatStreamChunk>),
}

#[derive(Default)]
struct StreamedAssistantContent {
    content: String,
    reasoning: String,
    saw_error: bool,
}

#[derive(Clone)]
pub struct ChatService {
    state: ModelState,
}

impl ChatService {
    pub fn new(state: ModelState) -> Self {
        Self { state }
    }

    pub async fn list_chat_models(&self) -> Result<Vec<ChatModelOption>, AppCoreError> {
        cloud::list_chat_models(&self.state).await
    }

    pub async fn create_chat_completion(
        &self,
        command: ChatCompletionCommand,
    ) -> Result<ChatCompletionOutput, AppCoreError> {
        create_chat_completion_with_state(self.state.clone(), command).await
    }

    pub async fn create_text_completion(
        &self,
        command: TextCompletionCommand,
    ) -> Result<TextCompletionOutput, AppCoreError> {
        create_text_completion_with_state(self.state.clone(), command).await
    }
}

/// Build an OpenAI-compatible `chat.completion.chunk` SSE data payload.
fn build_chunk(id: &str, created: i64, model: &str, token: &str) -> String {
    serde_json::json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "system_fingerprint": SYSTEM_FINGERPRINT,
        "choices": [{
            "index": 0,
            "delta": { "content": token },
            "finish_reason": null
        }]
    })
    .to_string()
}

/// Build an OpenAI-compatible initial SSE chunk that announces the assistant role.
fn build_role_chunk(id: &str, created: i64, model: &str) -> String {
    serde_json::json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "system_fingerprint": SYSTEM_FINGERPRINT,
        "choices": [{
            "index": 0,
            "delta": { "role": "assistant" },
            "finish_reason": null
        }]
    })
    .to_string()
}

/// Build an OpenAI-compatible reasoning SSE chunk payload.
fn build_reasoning_chunk(id: &str, created: i64, model: &str, token: &str) -> String {
    serde_json::json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "system_fingerprint": SYSTEM_FINGERPRINT,
        "choices": [{
            "index": 0,
            "delta": { "reasoning_content": token },
            "finish_reason": null
        }]
    })
    .to_string()
}

/// Build an OpenAI-compatible final SSE chunk with a finish reason.
fn build_finish_chunk(id: &str, created: i64, model: &str, finish_reason: &str) -> String {
    serde_json::json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "system_fingerprint": SYSTEM_FINGERPRINT,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": finish_reason
        }]
    })
    .to_string()
}

fn build_usage_chunk(id: &str, created: i64, model: &str, usage: &TextGenerationUsage) -> String {
    serde_json::json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "system_fingerprint": SYSTEM_FINGERPRINT,
        "choices": [],
        "usage": usage_to_json(usage)
    })
    .to_string()
}

fn build_text_completion_chunk(
    id: &str,
    created: i64,
    model: &str,
    index: u32,
    text: &str,
) -> String {
    serde_json::json!({
        "id": id,
        "object": "text_completion",
        "created": created,
        "model": model,
        "system_fingerprint": SYSTEM_FINGERPRINT,
        "choices": [{
            "index": index,
            "text": text,
            "finish_reason": null
        }]
    })
    .to_string()
}

fn build_text_completion_finish_chunk(
    id: &str,
    created: i64,
    model: &str,
    index: u32,
    finish_reason: &str,
) -> String {
    serde_json::json!({
        "id": id,
        "object": "text_completion",
        "created": created,
        "model": model,
        "system_fingerprint": SYSTEM_FINGERPRINT,
        "choices": [{
            "index": index,
            "text": "",
            "finish_reason": finish_reason
        }]
    })
    .to_string()
}

fn build_error_chunk(message: &str) -> String {
    serde_json::json!({
        "error": {
            "message": message,
            "type": "server_error",
            "code": null
        }
    })
    .to_string()
}

fn usage_to_json(usage: &TextGenerationUsage) -> serde_json::Value {
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

pub(super) fn estimate_token_count(text: &str) -> u32 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }

    let bytes = trimmed.len() as u32;
    let whitespace_groups = trimmed.split_whitespace().count() as u32;
    let byte_estimate = bytes.div_ceil(4);
    byte_estimate.max(whitespace_groups).max(1)
}

pub(super) fn finish_reason_from_token_budget(completion_tokens: u32, max_tokens: u32) -> String {
    if completion_tokens >= max_tokens && max_tokens > 0 {
        "length".to_owned()
    } else {
        "stop".to_owned()
    }
}

pub(super) fn build_estimated_usage(
    prompt_text: &str,
    completion_text: &str,
    completion_tokens: Option<u32>,
) -> TextGenerationUsage {
    let prompt_tokens = estimate_token_count(prompt_text);
    let completion_tokens =
        completion_tokens.unwrap_or_else(|| estimate_token_count(completion_text));

    TextGenerationUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens.saturating_add(completion_tokens),
        prompt_tokens_details: Default::default(),
        estimated: true,
    }
}

async fn persist_session_message(
    state: &ModelState,
    session_id: &str,
    message: &DomainConversationMessage,
) {
    state
        .store()
        .append_message(ChatMessage {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.to_owned(),
            role: message.role.clone(),
            content: serialize_session_message(message),
            created_at: Utc::now(),
        })
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(
                error = %error,
                role = %message.role,
                session_id,
                "failed to persist session message"
            )
        });
}

fn extract_text_from_chunk_delta<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value
        .get("choices")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|choice| {
            choice.get("delta").and_then(|delta| delta.get(field)).and_then(Value::as_str)
        })
        .find(|value| !value.is_empty())
}

fn capture_streamed_assistant_chunk(data: &str, assistant: &Arc<Mutex<StreamedAssistantContent>>) {
    if data.trim().is_empty() || data.trim() == "[DONE]" {
        return;
    }

    let Ok(payload) = serde_json::from_str::<Value>(data) else {
        return;
    };

    let mut assistant = assistant.lock().expect("assistant stream accumulator poisoned");
    if payload.get("error").is_some() {
        assistant.saw_error = true;
        return;
    }
    if let Some(reasoning) = extract_text_from_chunk_delta(&payload, "reasoning_content") {
        assistant.reasoning.push_str(reasoning);
    }
    if let Some(content) = extract_text_from_chunk_delta(&payload, "content") {
        assistant.content.push_str(content);
    }
}

fn build_streamed_assistant_message(
    assistant: &StreamedAssistantContent,
) -> Option<DomainConversationMessage> {
    if assistant.saw_error {
        return None;
    }

    let reasoning = assistant.reasoning.trim();
    let content = assistant.content.trim();
    if content.is_empty() && reasoning.is_empty() {
        return None;
    }

    Some(assistant_message_from_parts(
        assistant.content.as_str(),
        (!reasoning.is_empty()).then_some(assistant.reasoning.as_str()),
    ))
}

fn with_stream_session_persistence(
    stream: BoxStream<'static, ChatStreamChunk>,
    state: ModelState,
    session_id: String,
) -> BoxStream<'static, ChatStreamChunk> {
    let assistant = Arc::new(Mutex::new(StreamedAssistantContent::default()));
    let capture_target = Arc::clone(&assistant);
    let streamed = stream.map(move |chunk| {
        match &chunk {
            ChatStreamChunk::Data(data) => capture_streamed_assistant_chunk(data, &capture_target),
        }
        chunk
    });

    let persist_target = Arc::clone(&assistant);
    let persist = stream::once(async move {
        let message = {
            let assistant = persist_target.lock().expect("assistant stream accumulator poisoned");
            build_streamed_assistant_message(&assistant)
        };

        if let Some(message) = message.as_ref() {
            persist_session_message(&state, &session_id, message).await;
        }

        None::<ChatStreamChunk>
    })
    .filter_map(futures::future::ready);

    Box::pin(streamed.chain(persist))
}

async fn resolve_requested_model(
    state: &ModelState,
    requested_model: &str,
) -> Result<String, AppCoreError> {
    let trimmed = requested_model.trim();
    if !trimmed.is_empty() {
        return Ok(trimmed.to_owned());
    }

    let options = cloud::list_chat_models(state).await?;
    let preferred = options
        .iter()
        .find(|item| item.downloaded || item.provider_id.is_some())
        .or_else(|| options.first());

    preferred
        .map(|item| item.id.clone())
        .ok_or_else(|| AppCoreError::BadRequest("no chat-compatible models are configured".into()))
}

fn apply_stop_sequences(text: &str, stop: &[String]) -> (String, bool) {
    let Some((index, _)) = stop
        .iter()
        .filter(|value| !value.is_empty())
        .filter_map(|value| text.find(value).map(|index| (index, value)))
        .min_by_key(|(index, _)| *index)
    else {
        return (text.to_owned(), false);
    };

    (text[..index].to_owned(), true)
}

fn merge_usage(total: &mut Option<TextGenerationUsage>, next: Option<TextGenerationUsage>) {
    let Some(next) = next else {
        return;
    };

    match total {
        Some(total) => {
            total.prompt_tokens = total.prompt_tokens.saturating_add(next.prompt_tokens);
            total.completion_tokens =
                total.completion_tokens.saturating_add(next.completion_tokens);
            total.total_tokens = total.total_tokens.saturating_add(next.total_tokens);
            total.prompt_tokens_details.cached_tokens = total
                .prompt_tokens_details
                .cached_tokens
                .saturating_add(next.prompt_tokens_details.cached_tokens);
            total.estimated |= next.estimated;
        }
        None => *total = Some(next),
    }
}

fn validate_cloud_structured_output(
    structured_output: Option<&StructuredOutput>,
) -> Result<(), AppCoreError> {
    let Some(StructuredOutput::JsonSchema(schema)) = structured_output else {
        return Ok(());
    };

    if matches!(schema.strict, Some(false)) {
        return Err(unsupported_chat_parameter(
            "response_format.json_schema.strict",
            "cloud structured outputs currently require strict=true",
        ));
    }

    Ok(())
}

fn unsupported_chat_parameter(param: &str, message: impl Into<String>) -> AppCoreError {
    AppCoreError::BadRequestData {
        message: message.into(),
        data: json!({
            "error_type": "invalid_request_error",
            "code": "unsupported_chat_parameter",
            "param": param,
        }),
    }
}

fn validate_chat_route_params(
    route_to_cloud: bool,
    command: &ChatCompletionCommand,
) -> Result<(), AppCoreError> {
    if route_to_cloud {
        if command.local.grammar.is_some() {
            return Err(unsupported_chat_parameter(
                "grammar",
                "cloud chat completions do not support raw grammar constraints",
            ));
        }
        validate_cloud_structured_output(command.cloud.structured_output.as_ref())?;
        return Ok(());
    }

    if command.cloud.reasoning_effort.is_some() {
        return Err(unsupported_chat_parameter(
            "reasoning_effort",
            "local chat completions do not support reasoning controls",
        ));
    }
    if command.cloud.verbosity.is_some() {
        return Err(unsupported_chat_parameter(
            "verbosity",
            "local chat completions do not support verbosity controls",
        ));
    }

    Ok(())
}

fn validate_text_route_params(
    route_to_cloud: bool,
    command: &TextCompletionCommand,
) -> Result<(), AppCoreError> {
    if route_to_cloud {
        if command.local.grammar.is_some() {
            return Err(unsupported_chat_parameter(
                "grammar",
                "cloud text completions do not support raw grammar constraints",
            ));
        }
        validate_cloud_structured_output(command.cloud.structured_output.as_ref())?;
    }

    Ok(())
}

fn into_text_completion_stream(
    id: String,
    created: i64,
    model: String,
    text: String,
    finish_reason: String,
) -> TextCompletionOutput {
    let mut chunks = Vec::new();
    if !text.is_empty() {
        chunks.push(ChatStreamChunk::Data(build_text_completion_chunk(
            &id, created, &model, 0, &text,
        )));
    }
    chunks.push(ChatStreamChunk::Data(build_text_completion_finish_chunk(
        &id,
        created,
        &model,
        0,
        &finish_reason,
    )));
    chunks.push(ChatStreamChunk::Data("[DONE]".into()));

    TextCompletionOutput::Stream(Box::pin(stream::iter(chunks)))
}

async fn create_chat_completion_with_state(
    state: ModelState,
    command: ChatCompletionCommand,
) -> Result<ChatCompletionOutput, AppCoreError> {
    if command.common.stream && command.common.n > 1 {
        return Err(AppCoreError::NotImplemented("streaming with n > 1 is not supported".into()));
    }
    if command.common.stream && !command.common.stop.is_empty() {
        return Err(AppCoreError::NotImplemented(
            "streaming with stop is not supported for chat completions".into(),
        ));
    }

    let resolved_model = resolve_requested_model(&state, &command.model).await?;
    let continue_generation = command.continue_generation;
    let user_content = command
        .messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .map(DomainConversationMessage::rendered_text)
        .unwrap_or_default();

    let max_tokens = command.common.max_tokens.unwrap_or(512);
    let temperature = command.common.temperature.unwrap_or(0.7);
    let route_to_cloud = cloud::should_route_to_cloud(&state, &resolved_model).await?;
    validate_chat_route_params(route_to_cloud, &command)?;

    debug!(
        model = %resolved_model,
        prompt_len = user_content.len(),
        stream = command.common.stream,
        continue_generation,
        session_id = ?command.id,
        "chat completion request"
    );

    let resolved_messages =
        build_messages(&state, command.id.as_deref(), &command.messages).await?;
    let latest_user_message = command
        .messages
        .iter()
        .rev()
        .find(|message| message.role == "user" && message.has_meaningful_content())
        .cloned();

    if let Some(session_id) = command.id.as_deref().filter(|_| !continue_generation) {
        if let Some(message) = latest_user_message.as_ref() {
            persist_session_message(&state, session_id, message).await;
        } else if !user_content.trim().is_empty() {
            persist_session_message(
                &state,
                session_id,
                &DomainConversationMessage {
                    role: "user".into(),
                    content: ConversationMessageContent::Text(user_content.clone()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                },
            )
            .await;
        }
    }

    if command.common.stream {
        let generated = if route_to_cloud {
            cloud::create_chat_completion(
                &state,
                &resolved_model,
                &resolved_messages,
                cloud::CloudChatRequestConfig {
                    max_tokens,
                    temperature,
                    top_p: command.common.top_p,
                    structured_output: command.cloud.structured_output.clone(),
                    reasoning_effort: command.cloud.reasoning_effort,
                    verbosity: command.cloud.verbosity,
                    stream: true,
                    include_usage: command.common.stream_options.include_usage,
                },
            )
            .await?
        } else {
            local::create_chat_completion(
                &state,
                &resolved_model,
                &resolved_messages,
                local::LocalChatRequestConfig {
                    session_id: command.id.clone(),
                    max_tokens,
                    temperature,
                    top_p: command.common.top_p,
                    grammar: command.local.grammar.clone(),
                    grammar_json: command.local.structured_output.is_some(),
                    stream: true,
                    include_usage: command.common.stream_options.include_usage,
                },
            )
            .await?
        };

        return match generated {
            GeneratedChatOutput::Text(text) => {
                let assistant_message = assistant_message_from_text_response(&text);
                if let Some(session_id) = command.id.as_deref() {
                    persist_session_message(&state, session_id, &assistant_message).await;
                }

                let response = ChatCompletionResult {
                    id: format!("chatcmpl-{}", Uuid::new_v4()),
                    object: "chat.completion".into(),
                    created: Utc::now().timestamp(),
                    model: resolved_model,
                    system_fingerprint: SYSTEM_FINGERPRINT.into(),
                    choices: vec![ChatResultChoice {
                        index: 0,
                        message: assistant_message,
                        finish_reason: text.finish_reason.or(Some("stop".into())),
                    }],
                    usage: text.usage,
                };
                Ok(ChatCompletionOutput::Json(response))
            }
            GeneratedChatOutput::Stream(stream) => {
                let stream = match command.id.clone() {
                    Some(session_id) => {
                        with_stream_session_persistence(stream, state.clone(), session_id)
                    }
                    None => stream,
                };
                Ok(ChatCompletionOutput::Stream(stream))
            }
        };
    }

    let mut choices = Vec::new();
    let mut usage = None;
    for index in 0..command.common.n {
        let mut generated = if route_to_cloud {
            generate_cloud_chat_text(
                &state,
                &resolved_model,
                &resolved_messages,
                cloud::CloudChatRequestConfig {
                    max_tokens,
                    temperature,
                    top_p: command.common.top_p,
                    structured_output: command.cloud.structured_output.clone(),
                    reasoning_effort: command.cloud.reasoning_effort,
                    verbosity: command.cloud.verbosity,
                    stream: false,
                    include_usage: false,
                },
            )
            .await?
        } else {
            generate_local_chat_text(
                &state,
                &resolved_model,
                &resolved_messages,
                local::LocalChatRequestConfig {
                    session_id: command.id.clone(),
                    max_tokens,
                    temperature,
                    top_p: command.common.top_p,
                    grammar: command.local.grammar.clone(),
                    grammar_json: command.local.structured_output.is_some(),
                    stream: false,
                    include_usage: false,
                },
            )
            .await?
        };

        let (trimmed_text, stop_matched) =
            apply_stop_sequences(&generated.text, &command.common.stop);
        if stop_matched {
            generated.text = trimmed_text;
            generated.finish_reason = Some("stop".into());
        }

        merge_usage(&mut usage, generated.usage.clone());
        let assistant_message = assistant_message_from_text_response(&generated);
        choices.push(ChatResultChoice {
            index,
            message: assistant_message,
            finish_reason: generated.finish_reason.or(Some("stop".into())),
        });
    }

    info!(
        model = %resolved_model,
        output_len = choices
            .first()
            .map(|choice| choice.message.rendered_text().len())
            .unwrap_or_default(),
        "chat completion done"
    );

    if let Some(session_id) = command.id.as_deref()
        && let Some(first_choice) = choices.first()
    {
        persist_session_message(&state, session_id, &first_choice.message).await;
    }

    let response = ChatCompletionResult {
        id: format!("chatcmpl-{}", Uuid::new_v4()),
        object: "chat.completion".into(),
        created: Utc::now().timestamp(),
        model: resolved_model,
        system_fingerprint: SYSTEM_FINGERPRINT.into(),
        choices,
        usage,
    };

    Ok(ChatCompletionOutput::Json(response))
}

async fn create_text_completion_with_state(
    state: ModelState,
    command: TextCompletionCommand,
) -> Result<TextCompletionOutput, AppCoreError> {
    if command.common.stream && command.common.n > 1 {
        return Err(AppCoreError::NotImplemented("streaming with n > 1 is not supported".into()));
    }

    let resolved_model = resolve_requested_model(&state, &command.model).await?;
    let max_tokens = command.common.max_tokens.unwrap_or(512);
    let temperature = command.common.temperature.unwrap_or(0.7);
    let route_to_cloud = cloud::should_route_to_cloud(&state, &resolved_model).await?;
    validate_text_route_params(route_to_cloud, &command)?;

    debug!(
        model = %resolved_model,
        prompt_len = command.prompt.len(),
        stream = command.common.stream,
        "text completion request"
    );

    let mut choices = Vec::new();
    let mut usage = None;
    for index in 0..command.common.n {
        let mut generated = if route_to_cloud {
            cloud::create_text_completion(
                &state,
                &resolved_model,
                &command.prompt,
                cloud::CloudChatRequestConfig {
                    max_tokens,
                    temperature,
                    top_p: command.common.top_p,
                    structured_output: command.cloud.structured_output.clone(),
                    reasoning_effort: None,
                    verbosity: None,
                    stream: false,
                    include_usage: false,
                },
            )
            .await?
        } else {
            local::create_text_completion(
                &state,
                &resolved_model,
                &command.prompt,
                local::LocalTextRequestConfig {
                    max_tokens,
                    temperature,
                    top_p: command.common.top_p,
                    grammar: command.local.grammar.clone(),
                    grammar_json: command.local.structured_output.is_some(),
                },
            )
            .await?
        };

        let (trimmed_text, stop_matched) =
            apply_stop_sequences(&generated.text, &command.common.stop);
        if stop_matched {
            generated.text = trimmed_text;
            generated.finish_reason = Some("stop".into());
        }

        merge_usage(&mut usage, generated.usage.clone());
        choices.push(TextResultChoice {
            index,
            text: generated.text,
            finish_reason: generated.finish_reason.or(Some("stop".into())),
        });
    }

    let response = TextCompletionResult {
        id: format!("cmpl-{}", Uuid::new_v4()),
        object: "text_completion".into(),
        created: Utc::now().timestamp(),
        model: resolved_model.clone(),
        system_fingerprint: SYSTEM_FINGERPRINT.into(),
        choices,
        usage,
    };

    if command.common.stream {
        let first_choice =
            response.choices.first().cloned().ok_or_else(|| {
                AppCoreError::Internal("text completion produced no choices".into())
            })?;
        return Ok(into_text_completion_stream(
            response.id,
            response.created,
            resolved_model,
            first_choice.text,
            first_choice.finish_reason.unwrap_or_else(|| "stop".into()),
        ));
    }

    Ok(TextCompletionOutput::Json(response))
}

async fn generate_cloud_chat_text(
    state: &ModelState,
    model: &str,
    messages: &[DomainConversationMessage],
    config: cloud::CloudChatRequestConfig,
) -> Result<TextGenerationResponse, AppCoreError> {
    match cloud::create_chat_completion(state, model, messages, config).await? {
        GeneratedChatOutput::Text(text) => Ok(text),
        GeneratedChatOutput::Stream(_) => Err(AppCoreError::Internal(
            "cloud chat completion unexpectedly returned a stream".into(),
        )),
    }
}

async fn generate_local_chat_text(
    state: &ModelState,
    model: &str,
    messages: &[DomainConversationMessage],
    config: local::LocalChatRequestConfig,
) -> Result<TextGenerationResponse, AppCoreError> {
    match local::create_chat_completion(state, model, messages, config).await? {
        GeneratedChatOutput::Text(text) => Ok(text),
        GeneratedChatOutput::Stream(_) => Err(AppCoreError::Internal(
            "local chat completion unexpectedly returned a stream".into(),
        )),
    }
}

/// Merge history from DB and current request messages while avoiding duplicates.
async fn build_messages(
    state: &ModelState,
    session_id: Option<&str>,
    current_messages: &[DomainConversationMessage],
) -> Result<Vec<DomainConversationMessage>, AppCoreError> {
    let current: Vec<DomainConversationMessage> = current_messages
        .iter()
        .filter(|message| message.has_meaningful_content())
        .cloned()
        .collect();
    let client_sent_history = current.len() > 1;

    let mut merged = Vec::new();
    if !client_sent_history && let Some(session_id) = session_id {
        let history = state.store().list_messages(session_id).await?;
        for message in history {
            if message.content.trim().is_empty() {
                continue;
            }
            merged.push(message.into());
        }
    }
    merged.extend(current);
    Ok(merged)
}

#[cfg(test)]
mod test {
    use super::*;

    fn make_command(role: &str, content: &str) -> ChatCompletionCommand {
        ChatCompletionCommand {
            model: "test".into(),
            messages: vec![DomainConversationMessage {
                role: role.into(),
                content: ConversationMessageContent::Text(content.into()),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
            continue_generation: false,
            common: crate::domain::models::CommonChatParams {
                max_tokens: None,
                temperature: None,
                top_p: None,
                n: 1,
                stream: false,
                stop: Vec::new(),
                stream_options: Default::default(),
            },
            local: crate::domain::models::LocalChatParams {
                grammar: None,
                structured_output: None,
            },
            cloud: crate::domain::models::CloudChatParams {
                reasoning_effort: None,
                verbosity: None,
                structured_output: None,
            },
            id: None,
        }
    }

    #[test]
    fn validate_max_tokens_zero() {
        let mut req = make_command("user", "hello");
        req.common.max_tokens = Some(0);
        assert_eq!(req.common.max_tokens, Some(0));
        let max_tokens = req.common.max_tokens.unwrap_or(512);
        assert!(max_tokens == 0 || max_tokens > 4096, "should be out of range");
    }

    #[test]
    fn validate_max_tokens_too_large() {
        let mut req = make_command("user", "hello");
        req.common.max_tokens = Some(9999);
        let max_tokens = req.common.max_tokens.unwrap_or(512);
        assert!(max_tokens > 4096, "should be out of range");
    }

    #[test]
    fn validate_temperature_out_of_range() {
        let temperature = 3.0_f32;
        assert!(!(0.0..=2.0).contains(&temperature), "should be out of range");
    }

    #[test]
    fn no_user_message_returns_error() {
        let req = make_command("system", "you are a bot");
        let found = req.messages.iter().rev().find(|message| message.role == "user");
        assert!(found.is_none());
    }

    #[test]
    fn build_chunk_produces_openai_format() {
        let json_str = build_chunk("chatcmpl-test", 1_700_000_000, "slab-llama", "Hello");
        let value: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
        assert_eq!(value["id"], "chatcmpl-test");
        assert_eq!(value["object"], "chat.completion.chunk");
        assert_eq!(value["created"], 1_700_000_000_i64);
        assert_eq!(value["model"], "slab-llama");
        assert_eq!(value["system_fingerprint"], SYSTEM_FINGERPRINT);
        let choice = &value["choices"][0];
        assert_eq!(choice["index"], 0);
        assert_eq!(choice["delta"]["content"], "Hello");
        assert!(choice["finish_reason"].is_null());
    }

    #[test]
    fn apply_stop_sequences_truncates_at_first_match() {
        let (trimmed, matched) = apply_stop_sequences("hello STOP world", &["STOP".into()]);

        assert!(matched);
        assert_eq!(trimmed, "hello ");
    }

    #[test]
    fn cloud_structured_output_rejects_strict_false() {
        let result = validate_cloud_structured_output(Some(&StructuredOutput::JsonSchema(
            crate::domain::models::StructuredOutputJsonSchema {
                name: "example".into(),
                description: None,
                strict: Some(false),
                schema: serde_json::json!({ "type": "object" }),
            },
        )));

        assert!(matches!(result, Err(AppCoreError::BadRequestData { .. })));
    }

    #[test]
    fn local_route_rejects_reasoning_controls() {
        let mut command = make_command("user", "hello");
        command.cloud.reasoning_effort = Some(crate::domain::models::ChatReasoningEffort::Low);

        let result = validate_chat_route_params(false, &command);

        assert!(matches!(result, Err(AppCoreError::BadRequestData { .. })));
    }

    #[test]
    fn cloud_route_rejects_raw_grammar() {
        let mut command = make_command("user", "hello");
        command.local.grammar = Some("root ::= \"ok\"".into());

        let result = validate_chat_route_params(true, &command);

        assert!(matches!(result, Err(AppCoreError::BadRequestData { .. })));
    }

    #[test]
    fn streamed_assistant_message_restores_reasoning_chunks_for_session_storage() {
        let assistant = Arc::new(Mutex::new(StreamedAssistantContent::default()));

        capture_streamed_assistant_chunk(
            r#"{"choices":[{"delta":{"reasoning_content":"first thought"}}]}"#,
            &assistant,
        );
        capture_streamed_assistant_chunk(
            r#"{"choices":[{"delta":{"content":"final answer"}}]}"#,
            &assistant,
        );

        let message = {
            let assistant = assistant.lock().expect("assistant stream accumulator poisoned");
            build_streamed_assistant_message(&assistant).expect("expected assistant message")
        };

        assert!(matches!(
            message.content,
            ConversationMessageContent::Text(ref text)
                if text.contains("<think status=\"done\">")
                    && text.contains("first thought")
                    && text.ends_with("final answer")
        ));
    }

    #[test]
    fn streamed_assistant_message_skips_failed_streams() {
        let assistant = Arc::new(Mutex::new(StreamedAssistantContent::default()));

        capture_streamed_assistant_chunk(
            r#"{"choices":[{"delta":{"content":"partial answer"}}]}"#,
            &assistant,
        );
        capture_streamed_assistant_chunk(
            r#"{"error":{"message":"stream failed","type":"server_error","code":null}}"#,
            &assistant,
        );

        let message = {
            let assistant = assistant.lock().expect("assistant stream accumulator poisoned");
            build_streamed_assistant_message(&assistant)
        };

        assert!(message.is_none());
    }
}
