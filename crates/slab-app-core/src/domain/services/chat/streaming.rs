use std::sync::{Arc, Mutex};

use futures::StreamExt;
use futures::stream::{self, BoxStream};
use serde::Serialize;
use serde_json::Value;
use tracing::warn;

use crate::context::ModelState;
use crate::domain::models::{
    ChatStreamChunk, ConversationMessage as DomainConversationMessage, TextCompletionOutput,
    TextGenerationUsage, assistant_message_from_parts,
};

use super::SYSTEM_FINGERPRINT;
use super::session::persist_session_message;

#[derive(Default)]
struct StreamedAssistantContent {
    content: String,
    reasoning: String,
    usage: Option<TextGenerationUsage>,
    saw_error: bool,
}

#[derive(Serialize)]
struct ChatCompletionChunkPayload<'a> {
    id: &'a str,
    #[serde(rename = "object")]
    object_type: &'static str,
    created: i64,
    model: &'a str,
    system_fingerprint: &'static str,
    choices: Vec<ChatCompletionChunkChoice<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<&'a TextGenerationUsage>,
}

#[derive(Serialize)]
struct ChatCompletionChunkChoice<'a> {
    index: u32,
    delta: ChatCompletionChunkDelta<'a>,
    finish_reason: Option<&'a str>,
}

#[derive(Default, Serialize)]
struct ChatCompletionChunkDelta<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<&'a str>,
}

#[derive(Serialize)]
struct TextCompletionChunkPayload<'a> {
    id: &'a str,
    #[serde(rename = "object")]
    object_type: &'static str,
    created: i64,
    model: &'a str,
    system_fingerprint: &'static str,
    choices: Vec<TextCompletionChunkChoice<'a>>,
}

#[derive(Serialize)]
struct TextCompletionChunkChoice<'a> {
    index: u32,
    text: &'a str,
    finish_reason: Option<&'a str>,
}

#[derive(Serialize)]
struct ChatStreamErrorPayload<'a> {
    error: ChatStreamErrorBody<'a>,
}

#[derive(Serialize)]
struct ChatStreamErrorBody<'a> {
    message: &'a str,
    #[serde(rename = "type")]
    error_type: &'static str,
    code: Option<&'static str>,
}

/// Build an OpenAI-compatible `chat.completion.chunk` SSE data payload.
pub(super) fn build_chunk(id: &str, created: i64, model: &str, token: &str) -> String {
    serialize_chunk(&ChatCompletionChunkPayload {
        id,
        object_type: "chat.completion.chunk",
        created,
        model,
        system_fingerprint: SYSTEM_FINGERPRINT,
        choices: vec![ChatCompletionChunkChoice {
            index: 0,
            delta: ChatCompletionChunkDelta { content: Some(token), ..Default::default() },
            finish_reason: None,
        }],
        usage: None,
    })
}

/// Build an OpenAI-compatible initial SSE chunk that announces the assistant role.
pub(super) fn build_role_chunk(id: &str, created: i64, model: &str) -> String {
    serialize_chunk(&ChatCompletionChunkPayload {
        id,
        object_type: "chat.completion.chunk",
        created,
        model,
        system_fingerprint: SYSTEM_FINGERPRINT,
        choices: vec![ChatCompletionChunkChoice {
            index: 0,
            delta: ChatCompletionChunkDelta { role: Some("assistant"), ..Default::default() },
            finish_reason: None,
        }],
        usage: None,
    })
}

/// Build an OpenAI-compatible reasoning SSE chunk payload.
pub(super) fn build_reasoning_chunk(id: &str, created: i64, model: &str, token: &str) -> String {
    serialize_chunk(&ChatCompletionChunkPayload {
        id,
        object_type: "chat.completion.chunk",
        created,
        model,
        system_fingerprint: SYSTEM_FINGERPRINT,
        choices: vec![ChatCompletionChunkChoice {
            index: 0,
            delta: ChatCompletionChunkDelta {
                reasoning_content: Some(token),
                ..Default::default()
            },
            finish_reason: None,
        }],
        usage: None,
    })
}

/// Build an OpenAI-compatible final SSE chunk with a finish reason.
pub(super) fn build_finish_chunk(
    id: &str,
    created: i64,
    model: &str,
    finish_reason: &str,
) -> String {
    serialize_chunk(&ChatCompletionChunkPayload {
        id,
        object_type: "chat.completion.chunk",
        created,
        model,
        system_fingerprint: SYSTEM_FINGERPRINT,
        choices: vec![ChatCompletionChunkChoice {
            index: 0,
            delta: ChatCompletionChunkDelta::default(),
            finish_reason: Some(finish_reason),
        }],
        usage: None,
    })
}

pub(super) fn build_usage_chunk(
    id: &str,
    created: i64,
    model: &str,
    usage: &TextGenerationUsage,
) -> String {
    serialize_chunk(&ChatCompletionChunkPayload {
        id,
        object_type: "chat.completion.chunk",
        created,
        model,
        system_fingerprint: SYSTEM_FINGERPRINT,
        choices: Vec::new(),
        usage: Some(usage),
    })
}

fn build_text_completion_chunk(
    id: &str,
    created: i64,
    model: &str,
    index: u32,
    text: &str,
) -> String {
    serialize_chunk(&TextCompletionChunkPayload {
        id,
        object_type: "text_completion",
        created,
        model,
        system_fingerprint: SYSTEM_FINGERPRINT,
        choices: vec![TextCompletionChunkChoice { index, text, finish_reason: None }],
    })
}

fn build_text_completion_finish_chunk(
    id: &str,
    created: i64,
    model: &str,
    index: u32,
    finish_reason: &str,
) -> String {
    serialize_chunk(&TextCompletionChunkPayload {
        id,
        object_type: "text_completion",
        created,
        model,
        system_fingerprint: SYSTEM_FINGERPRINT,
        choices: vec![TextCompletionChunkChoice {
            index,
            text: "",
            finish_reason: Some(finish_reason),
        }],
    })
}

pub(super) fn build_error_chunk(message: &str) -> String {
    serialize_chunk(&ChatStreamErrorPayload {
        error: ChatStreamErrorBody { message, error_type: "server_error", code: None },
    })
}

fn serialize_chunk<T: Serialize>(payload: &T) -> String {
    serde_json::to_string(payload)
        .unwrap_or_else(|_| r#"{"error":{"message":"failed to serialize stream chunk","type":"server_error","code":null}}"#.to_owned())
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

fn extract_usage_from_chunk(value: &Value) -> Option<TextGenerationUsage> {
    serde_json::from_value(value.get("usage")?.clone()).ok()
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
    if let Some(usage) = extract_usage_from_chunk(&payload) {
        assistant.usage = Some(usage);
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

pub(super) fn with_stream_session_persistence(
    stream: BoxStream<'static, ChatStreamChunk>,
    state: ModelState,
    session_id: String,
) -> BoxStream<'static, ChatStreamChunk> {
    let assistant = Arc::new(Mutex::new(StreamedAssistantContent::default()));
    let capture_target = Arc::clone(&assistant);
    let streamed = stream.map(move |chunk| {
        capture_streamed_assistant_chunk(&chunk, &capture_target);
        chunk
    });

    let persist_target = Arc::clone(&assistant);
    let persist = stream::once(async move {
        let (message, saw_error, content_len, reasoning_len, usage) = {
            let assistant = persist_target.lock().expect("assistant stream accumulator poisoned");
            (
                build_streamed_assistant_message(&assistant),
                assistant.saw_error,
                assistant.content.trim().len(),
                assistant.reasoning.trim().len(),
                assistant.usage.clone(),
            )
        };

        if message.is_none() && !saw_error {
            warn!(
                session_id = %session_id,
                content_len,
                reasoning_len,
                prompt_tokens = usage.as_ref().map(|value| value.prompt_tokens).unwrap_or(0),
                completion_tokens = usage.as_ref().map(|value| value.completion_tokens).unwrap_or(0),
                total_tokens = usage.as_ref().map(|value| value.total_tokens).unwrap_or(0),
                usage_estimated = usage.as_ref().map(|value| value.estimated).unwrap_or(true),
                "chat stream completed without visible assistant output"
            );
        }

        if let Some(message) = message.as_ref() {
            persist_session_message(&state, &session_id, message).await;
        }

        None::<ChatStreamChunk>
    })
    .filter_map(futures::future::ready);

    Box::pin(streamed.chain(persist))
}

pub(super) fn into_text_completion_stream(
    id: String,
    created: i64,
    model: String,
    text: String,
    finish_reason: String,
) -> TextCompletionOutput {
    let mut chunks = Vec::new();
    if !text.is_empty() {
        chunks.push(build_text_completion_chunk(&id, created, &model, 0, &text));
    }
    chunks.push(build_text_completion_finish_chunk(&id, created, &model, 0, &finish_reason));
    chunks.push("[DONE]".into());

    TextCompletionOutput::Stream(Box::pin(stream::iter(chunks)))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::domain::models::ConversationMessageContent;

    use super::{StreamedAssistantContent, build_chunk};
    use super::{build_streamed_assistant_message, capture_streamed_assistant_chunk};
    use crate::domain::services::chat::SYSTEM_FINGERPRINT;

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
