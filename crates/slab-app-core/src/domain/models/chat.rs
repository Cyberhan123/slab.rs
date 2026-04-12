pub use slab_types::chat::{
    ChatModelCapabilities, ChatModelSource, ChatReasoningEffort, ChatVerbosity,
    ConversationContentPart, ConversationMessage, ConversationMessageContent, ConversationToolCall,
    ConversationToolFunction,
};
pub use slab_types::inference::{TextGenerationResponse, TextGenerationUsage};

use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::models::ManagedModelBackendId;

const SESSION_MESSAGE_STORAGE_VERSION: u8 = 2;
const SESSION_MESSAGE_STORAGE_KIND: &str = "conversation_message";
const REASONING_CONTENT_METADATA_KEY: &str = "reasoning_content";

pub enum ChatStreamChunk {
    Data(String),
}

pub enum ChatCompletionOutput {
    Json(ChatCompletionResult),
    Stream(BoxStream<'static, ChatStreamChunk>),
}

pub enum TextCompletionOutput {
    Json(TextCompletionResult),
    Stream(BoxStream<'static, ChatStreamChunk>),
}

#[derive(Debug, Clone)]
pub struct ChatModelOption {
    pub id: String,
    pub display_name: String,
    pub source: ChatModelSource,
    pub downloaded: bool,
    pub pending: bool,
    pub capabilities: ChatModelCapabilities,
    pub backend_id: Option<ManagedModelBackendId>,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CommonChatParams {
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub n: u32,
    pub stream: bool,
    pub stop: Vec<String>,
    pub stream_options: ChatStreamOptions,
}

#[derive(Debug, Clone, Default)]
pub struct LocalChatParams {
    pub grammar: Option<String>,
    pub structured_output: Option<StructuredOutput>,
}

#[derive(Debug, Clone, Default)]
pub struct CloudChatParams {
    pub reasoning_effort: Option<ChatReasoningEffort>,
    pub verbosity: Option<ChatVerbosity>,
    pub structured_output: Option<StructuredOutput>,
}

#[derive(Debug, Clone)]
pub struct ChatCompletionCommand {
    pub id: Option<String>,
    pub model: String,
    pub messages: Vec<ConversationMessage>,
    pub continue_generation: bool,
    pub common: CommonChatParams,
    pub local: LocalChatParams,
    pub cloud: CloudChatParams,
}

#[derive(Debug, Clone)]
pub struct TextCompletionCommand {
    pub model: String,
    pub prompt: String,
    pub common: CommonChatParams,
    pub local: LocalChatParams,
    pub cloud: CloudChatParams,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StructuredOutput {
    JsonObject,
    JsonSchema(StructuredOutputJsonSchema),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructuredOutputJsonSchema {
    pub name: String,
    pub description: Option<String>,
    pub strict: Option<bool>,
    pub schema: Value,
}

impl StructuredOutputJsonSchema {
    pub fn new(
        name: Option<String>,
        description: Option<String>,
        strict: Option<bool>,
        schema: Value,
    ) -> Self {
        Self {
            name: sanitize_structured_output_name(name.as_deref()),
            description: normalize_optional_text(description),
            strict,
            schema,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ChatStreamOptions {
    pub include_usage: bool,
}

impl Default for ChatStreamOptions {
    fn default() -> Self {
        Self { include_usage: true }
    }
}

#[derive(Debug, Clone)]
pub struct ChatResultChoice {
    pub index: u32,
    pub message: ConversationMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChatCompletionResult {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub system_fingerprint: String,
    pub choices: Vec<ChatResultChoice>,
    pub usage: Option<TextGenerationUsage>,
}

#[derive(Debug, Clone)]
pub struct TextResultChoice {
    pub index: u32,
    pub text: String,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TextCompletionResult {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub system_fingerprint: String,
    pub choices: Vec<TextResultChoice>,
    pub usage: Option<TextGenerationUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSessionMessageV1 {
    version: u8,
    message: ConversationMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSessionMessageV2 {
    version: u8,
    kind: String,
    message: ConversationMessage,
}

pub fn serialize_session_message(message: &ConversationMessage) -> String {
    serde_json::to_string(&StoredSessionMessageV2 {
        version: SESSION_MESSAGE_STORAGE_VERSION,
        kind: SESSION_MESSAGE_STORAGE_KIND.to_owned(),
        message: message.clone(),
    })
    .unwrap_or_else(|_| message.rendered_text())
}

pub fn deserialize_session_message(role: &str, content: &str) -> ConversationMessage {
    let trimmed = content.trim();

    if let Ok(stored) = serde_json::from_str::<StoredSessionMessageV2>(trimmed)
        && stored.version == SESSION_MESSAGE_STORAGE_VERSION
        && stored.kind == SESSION_MESSAGE_STORAGE_KIND
    {
        return stored.message;
    }

    if let Ok(stored) = serde_json::from_str::<StoredSessionMessageV1>(trimmed)
        && stored.version == 1
    {
        return stored.message;
    }

    if let Ok(message) = serde_json::from_str::<ConversationMessage>(trimmed) {
        return message;
    }

    if let Ok(parsed_content) = serde_json::from_str::<ConversationMessageContent>(trimmed) {
        return ConversationMessage {
            role: role.to_owned(),
            content: parsed_content,
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        };
    }

    ConversationMessage {
        role: role.to_owned(),
        content: ConversationMessageContent::Text(content.to_owned()),
        name: None,
        tool_call_id: None,
        tool_calls: Vec::new(),
    }
}

pub fn assistant_message_from_text_response(
    response: &TextGenerationResponse,
) -> ConversationMessage {
    let reasoning = response
        .metadata
        .get(REASONING_CONTENT_METADATA_KEY)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    ConversationMessage {
        role: "assistant".into(),
        content: ConversationMessageContent::Text(format_assistant_content(
            reasoning,
            response.text.trim_end_matches('\0'),
        )),
        name: None,
        tool_call_id: None,
        tool_calls: Vec::new(),
    }
}

pub fn assistant_message_from_parts(content: &str, reasoning: Option<&str>) -> ConversationMessage {
    ConversationMessage {
        role: "assistant".into(),
        content: ConversationMessageContent::Text(format_assistant_content(reasoning, content)),
        name: None,
        tool_call_id: None,
        tool_calls: Vec::new(),
    }
}

fn format_assistant_content(reasoning: Option<&str>, content: &str) -> String {
    let trimmed_content = content.trim_end();
    let Some(reasoning) = reasoning.map(str::trim).filter(|value| !value.is_empty()) else {
        return trimmed_content.to_owned();
    };

    if trimmed_content.is_empty() {
        return format!("<think status=\"done\">\n\n{reasoning}\n\n</think>");
    }

    format!("<think status=\"done\">\n\n{reasoning}\n\n</think>\n\n{trimmed_content}")
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
    })
}

const DEFAULT_STRUCTURED_OUTPUT_NAME: &str = "slab_structured_output";

fn sanitize_structured_output_name(value: Option<&str>) -> String {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return DEFAULT_STRUCTURED_OUTPUT_NAME.to_owned();
    };

    let mut sanitized = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            sanitized.push(ch);
        } else if !sanitized.ends_with('_') {
            sanitized.push('_');
        }
    }

    let sanitized = sanitized.trim_matches('_');
    if sanitized.is_empty() {
        DEFAULT_STRUCTURED_OUTPUT_NAME.to_owned()
    } else {
        sanitized.to_owned()
    }
}

#[cfg(test)]
mod test {
    use super::{
        ConversationContentPart, ConversationMessage, ConversationMessageContent,
        ConversationToolCall, ConversationToolFunction,
    };
    use serde_json::json;
    use slab_types::inference::TextGenerationResponse;

    #[test]
    fn structured_session_messages_round_trip_through_json_storage() {
        let message = ConversationMessage {
            role: "assistant".into(),
            content: ConversationMessageContent::Parts(vec![
                ConversationContentPart::OutputText { text: "template reply".into() },
                ConversationContentPart::Json {
                    value: json!({
                        "kind": "chat_template",
                        "items": ["alpha", 1],
                    }),
                },
            ]),
            name: Some("planner".into()),
            tool_call_id: None,
            tool_calls: vec![ConversationToolCall {
                id: Some("call-1".into()),
                r#type: "function".into(),
                function: ConversationToolFunction {
                    name: "save_template".into(),
                    arguments: "{\"mode\":\"json\"}".into(),
                },
            }],
        };

        let stored = super::serialize_session_message(&message);
        let restored = super::deserialize_session_message("assistant", &stored);

        assert!(serde_json::from_str::<serde_json::Value>(&stored).is_ok());
        assert_eq!(restored, message);
    }

    #[test]
    fn plain_text_messages_now_persist_as_v2_json_envelope() {
        let message = ConversationMessage {
            role: "assistant".into(),
            content: ConversationMessageContent::Text("hello".into()),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        };

        let stored = super::serialize_session_message(&message);
        let payload: serde_json::Value = serde_json::from_str(&stored).expect("json envelope");

        assert_eq!(payload["version"], 2);
        assert_eq!(payload["kind"], "conversation_message");
        assert_eq!(super::deserialize_session_message("assistant", &stored), message);
    }

    #[test]
    fn content_only_json_payload_restores_with_fallback_role() {
        let content = ConversationMessageContent::Parts(vec![
            ConversationContentPart::InputText { text: "hello".into() },
            ConversationContentPart::Json {
                value: json!({
                    "kind": "chat_template",
                    "version": 1,
                }),
            },
        ]);
        let stored = serde_json::to_string(&content).expect("content should serialize");

        let restored = super::deserialize_session_message("user", &stored);

        assert_eq!(restored.role, "user");
        assert_eq!(restored.content, content);
    }

    #[test]
    fn assistant_reasoning_is_embedded_in_session_text_content() {
        let mut metadata = slab_types::inference::JsonOptions::default();
        metadata.insert("reasoning_content".into(), json!("step by step"));
        let response = TextGenerationResponse {
            text: "final answer".into(),
            finish_reason: Some("stop".into()),
            tokens_used: None,
            usage: None,
            metadata,
        };

        let message = super::assistant_message_from_text_response(&response);

        assert!(matches!(
            message.content,
            ConversationMessageContent::Text(ref text)
                if text.contains("<think status=\"done\">")
                    && text.contains("step by step")
                    && text.ends_with("final answer")
        ));

        let stored = super::serialize_session_message(&message);
        assert!(stored.contains("step by step"));
        assert_eq!(super::deserialize_session_message("assistant", &stored), message);
    }
}
