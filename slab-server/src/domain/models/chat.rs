pub use slab_types::chat::{
    ChatModelSource, ChatReasoningEffort, ChatVerbosity, ConversationContentPart,
    ConversationMessage, ConversationMessageContent, ConversationToolCall,
    ConversationToolFunction,
};
pub use slab_types::inference::{TextGenerationResponse, TextGenerationUsage};

use crate::api::v1::chat::schema::{
    ChatCompletionRequest, ChatReasoningEffort as ApiChatReasoningEffort,
    ChatResponseFormat as ApiChatResponseFormat, ChatResponseFormatType, ChatResponseJsonSchema,
    ChatStreamOptions as ApiChatStreamOptions, ChatThinkingConfig as ApiChatThinkingConfig,
    ChatThinkingType as ApiChatThinkingType, ChatVerbosity as ApiChatVerbosity, CompletionRequest,
};
use crate::infra::db;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const SESSION_MESSAGE_STORAGE_VERSION: u8 = 1;
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
    pub backend_id: Option<String>,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChatCompletionCommand {
    pub id: Option<String>,
    pub model: String,
    pub messages: Vec<ConversationMessage>,
    pub continue_generation: bool,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub n: u32,
    pub stop: Vec<String>,
    pub grammar: Option<String>,
    pub grammar_json: bool,
    pub structured_output: Option<StructuredOutput>,
    pub reasoning_effort: Option<ChatReasoningEffort>,
    pub verbosity: Option<ChatVerbosity>,
    pub stream: bool,
    pub stream_options: ChatStreamOptions,
}

#[derive(Debug, Clone)]
pub struct TextCompletionCommand {
    pub model: String,
    pub prompt: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub n: u32,
    pub stop: Vec<String>,
    pub grammar: Option<String>,
    pub grammar_json: bool,
    pub structured_output: Option<StructuredOutput>,
    pub stream: bool,
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
    fn new(
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
struct StoredSessionMessage {
    version: u8,
    message: ConversationMessage,
}

impl From<db::ChatMessage> for ConversationMessage {
    fn from(message: db::ChatMessage) -> Self {
        deserialize_session_message(&message.role, &message.content)
    }
}

pub fn serialize_session_message(message: &ConversationMessage) -> String {
    if can_store_as_plain_text(message) {
        return match &message.content {
            ConversationMessageContent::Text(text) => text.clone(),
            ConversationMessageContent::Parts(_) => String::new(),
        };
    }

    serde_json::to_string(&StoredSessionMessage {
        version: SESSION_MESSAGE_STORAGE_VERSION,
        message: message.clone(),
    })
    .unwrap_or_else(|_| message.rendered_text())
}

pub fn deserialize_session_message(role: &str, content: &str) -> ConversationMessage {
    let trimmed = content.trim();

    if let Ok(stored) = serde_json::from_str::<StoredSessionMessage>(trimmed)
        && stored.version == SESSION_MESSAGE_STORAGE_VERSION
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

fn can_store_as_plain_text(message: &ConversationMessage) -> bool {
    matches!(message.content, ConversationMessageContent::Text(_))
        && message.name.is_none()
        && message.tool_call_id.is_none()
        && message.tool_calls.is_empty()
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

impl From<ApiChatReasoningEffort> for ChatReasoningEffort {
    fn from(value: ApiChatReasoningEffort) -> Self {
        match value {
            ApiChatReasoningEffort::None => Self::None,
            ApiChatReasoningEffort::Low => Self::Low,
            ApiChatReasoningEffort::Medium => Self::Medium,
            ApiChatReasoningEffort::High => Self::High,
            ApiChatReasoningEffort::Minimal => Self::Minimal,
        }
    }
}

impl From<ApiChatVerbosity> for ChatVerbosity {
    fn from(value: ApiChatVerbosity) -> Self {
        match value {
            ApiChatVerbosity::Low => Self::Low,
            ApiChatVerbosity::Medium => Self::Medium,
            ApiChatVerbosity::High => Self::High,
        }
    }
}

impl From<ApiChatStreamOptions> for ChatStreamOptions {
    fn from(value: ApiChatStreamOptions) -> Self {
        Self { include_usage: value.include_usage }
    }
}

impl From<ChatCompletionRequest> for ChatCompletionCommand {
    fn from(request: ChatCompletionRequest) -> Self {
        let ChatCompletionRequest {
            id,
            model,
            messages,
            continue_generation,
            stream,
            stream_options,
            max_tokens,
            temperature,
            top_p,
            n,
            stop,
            grammar,
            response_format,
            json_schema,
            thinking,
            reasoning_effort,
            verbosity,
        } = request;

        let reasoning_effort = reasoning_effort
            .map(Into::into)
            .or_else(|| thinking.as_ref().and_then(reasoning_effort_from_thinking));
        let verbosity = verbosity
            .map(Into::into)
            .or_else(|| thinking.as_ref().and_then(verbosity_from_thinking));
        let structured_output = structured_output_from_api(response_format, json_schema);
        let grammar_json = structured_output.is_some();
        let stop = stop
            .as_ref()
            .map(crate::api::v1::chat::schema::StopSequences::normalized)
            .unwrap_or_default();

        Self {
            id,
            model: model.trim().to_owned(),
            messages: messages.into_iter().map(Into::into).collect(),
            continue_generation,
            max_tokens,
            temperature,
            top_p,
            n: n.unwrap_or(1),
            stop,
            grammar,
            grammar_json,
            structured_output,
            reasoning_effort,
            verbosity,
            stream,
            stream_options: stream_options.map(Into::into).unwrap_or_default(),
        }
    }
}

fn reasoning_effort_from_thinking(thinking: &ApiChatThinkingConfig) -> Option<ChatReasoningEffort> {
    match thinking.mode {
        ApiChatThinkingType::Disabled => Some(ChatReasoningEffort::None),
        ApiChatThinkingType::Enabled => {
            thinking.reasoning_effort.map(Into::into).or(Some(ChatReasoningEffort::Medium))
        }
    }
}

fn verbosity_from_thinking(thinking: &ApiChatThinkingConfig) -> Option<ChatVerbosity> {
    match thinking.mode {
        ApiChatThinkingType::Disabled => None,
        ApiChatThinkingType::Enabled => thinking.verbosity.map(Into::into),
    }
}

impl From<CompletionRequest> for TextCompletionCommand {
    fn from(request: CompletionRequest) -> Self {
        let CompletionRequest {
            model,
            prompt,
            max_tokens,
            temperature,
            top_p,
            n,
            stop,
            stream,
            grammar,
            response_format,
            json_schema,
        } = request;
        let structured_output = structured_output_from_api(response_format, json_schema);
        let grammar_json = structured_output.is_some();
        let stop = stop
            .as_ref()
            .map(crate::api::v1::chat::schema::StopSequences::normalized)
            .unwrap_or_default();

        Self {
            model: model.trim().to_owned(),
            prompt,
            max_tokens,
            temperature,
            top_p,
            n: n.unwrap_or(1),
            stop,
            grammar,
            grammar_json,
            structured_output,
            stream,
        }
    }
}

const DEFAULT_STRUCTURED_OUTPUT_NAME: &str = "slab_structured_output";

fn structured_output_from_api(
    response_format: Option<ApiChatResponseFormat>,
    json_schema: Option<Value>,
) -> Option<StructuredOutput> {
    if let Some(schema) = json_schema {
        return Some(StructuredOutput::JsonSchema(StructuredOutputJsonSchema::new(
            None, None, None, schema,
        )));
    }

    let response_format = response_format?;
    match response_format.format_type {
        ChatResponseFormatType::Text => None,
        ChatResponseFormatType::JsonObject => response_format
            .json_schema
            .map(structured_output_json_schema_from_api)
            .map(StructuredOutput::JsonSchema)
            .or_else(|| {
                response_format.schema.map(|schema| {
                    StructuredOutput::JsonSchema(StructuredOutputJsonSchema::new(
                        None, None, None, schema,
                    ))
                })
            })
            .or(Some(StructuredOutput::JsonObject)),
        ChatResponseFormatType::JsonSchema => response_format
            .json_schema
            .map(structured_output_json_schema_from_api)
            .map(StructuredOutput::JsonSchema)
            .or_else(|| {
                response_format.schema.map(|schema| {
                    StructuredOutput::JsonSchema(StructuredOutputJsonSchema::new(
                        None, None, None, schema,
                    ))
                })
            })
            .or(Some(StructuredOutput::JsonObject)),
    }
}

fn structured_output_json_schema_from_api(
    value: ChatResponseJsonSchema,
) -> StructuredOutputJsonSchema {
    StructuredOutputJsonSchema::new(value.name, value.description, value.strict, value.schema)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
    })
}

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
        ChatCompletionCommand, ChatReasoningEffort, ChatVerbosity, ConversationContentPart,
        ConversationMessage, ConversationMessageContent, ConversationToolCall,
        ConversationToolFunction, StructuredOutput, TextCompletionCommand,
    };
    use crate::api::v1::chat::schema::{
        ChatCompletionRequest, ChatMessage, ChatResponseFormat, ChatResponseFormatType,
        ChatResponseJsonSchema, ChatStreamOptions, ChatThinkingConfig, ChatThinkingType,
        CompletionRequest,
    };
    use serde_json::json;
    use slab_types::inference::TextGenerationResponse;

    fn make_request() -> ChatCompletionRequest {
        ChatCompletionRequest {
            id: None,
            model: "cloud/provider/model".to_owned(),
            messages: vec![ChatMessage {
                role: "user".to_owned(),
                content: Some(crate::api::v1::chat::schema::ChatMessageContent::Text(
                    "hello".to_owned(),
                )),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
            continue_generation: false,
            stream: true,
            stream_options: Some(ChatStreamOptions { include_usage: true }),
            max_tokens: None,
            temperature: None,
            top_p: None,
            n: None,
            stop: None,
            grammar: None,
            response_format: None,
            json_schema: None,
            thinking: None,
            reasoning_effort: None,
            verbosity: None,
        }
    }

    fn make_completion_request() -> CompletionRequest {
        CompletionRequest {
            model: "cloud/provider/model".to_owned(),
            prompt: "hello".to_owned(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            n: None,
            stop: None,
            stream: false,
            grammar: None,
            response_format: None,
            json_schema: None,
        }
    }

    #[test]
    fn thinking_disabled_maps_to_reasoning_none() {
        let mut request = make_request();
        request.thinking = Some(ChatThinkingConfig {
            mode: ChatThinkingType::Disabled,
            reasoning_effort: None,
            verbosity: None,
        });

        let command = ChatCompletionCommand::from(request);

        assert!(matches!(command.reasoning_effort, Some(ChatReasoningEffort::None)));
    }

    #[test]
    fn thinking_enabled_defaults_to_medium_reasoning() {
        let mut request = make_request();
        request.thinking = Some(ChatThinkingConfig {
            mode: ChatThinkingType::Enabled,
            reasoning_effort: None,
            verbosity: None,
        });

        let command = ChatCompletionCommand::from(request);

        assert!(matches!(command.reasoning_effort, Some(ChatReasoningEffort::Medium)));
    }

    #[test]
    fn explicit_reasoning_and_verbosity_take_precedence() {
        let mut request = make_request();
        request.thinking = Some(ChatThinkingConfig {
            mode: ChatThinkingType::Disabled,
            reasoning_effort: None,
            verbosity: None,
        });
        request.reasoning_effort = Some(crate::api::v1::chat::schema::ChatReasoningEffort::High);
        request.verbosity = Some(crate::api::v1::chat::schema::ChatVerbosity::Low);

        let command = ChatCompletionCommand::from(request);

        assert!(matches!(command.reasoning_effort, Some(ChatReasoningEffort::High)));
        assert!(matches!(command.verbosity, Some(ChatVerbosity::Low)));
    }

    #[test]
    fn continue_generation_flag_is_preserved() {
        let mut request = make_request();
        request.continue_generation = true;

        let command = ChatCompletionCommand::from(request);

        assert!(command.continue_generation);
    }

    #[test]
    fn response_format_json_object_maps_to_grammar_json() {
        let mut request = make_request();
        request.response_format = Some(ChatResponseFormat {
            format_type: ChatResponseFormatType::JsonObject,
            schema: None,
            json_schema: None,
        });

        let command = ChatCompletionCommand::from(request);

        assert!(command.grammar_json);
        assert_eq!(command.structured_output, Some(StructuredOutput::JsonObject));
    }

    #[test]
    fn response_format_schema_maps_to_structured_json_schema() {
        let mut request = make_request();
        request.response_format = Some(ChatResponseFormat {
            format_type: ChatResponseFormatType::JsonObject,
            schema: Some(json!({ "const": "42" })),
            json_schema: None,
        });

        let command = ChatCompletionCommand::from(request);

        assert!(command.grammar_json);
        assert!(matches!(
            command.structured_output,
            Some(StructuredOutput::JsonSchema(ref schema)) if schema.schema == json!({ "const": "42" })
        ));
    }

    #[test]
    fn json_schema_field_maps_to_grammar_json() {
        let mut request = make_request();
        request.json_schema = Some(json!({ "const": "42" }));

        let command = ChatCompletionCommand::from(request);

        assert!(command.grammar_json);
        assert!(matches!(
            command.structured_output,
            Some(StructuredOutput::JsonSchema(ref schema)) if schema.name == "slab_structured_output"
        ));
    }

    #[test]
    fn response_format_json_schema_preserves_metadata() {
        let mut request = make_request();
        request.response_format = Some(ChatResponseFormat {
            format_type: ChatResponseFormatType::JsonSchema,
            schema: None,
            json_schema: Some(ChatResponseJsonSchema {
                name: Some("team schema/v1".to_owned()),
                description: Some("  example schema  ".to_owned()),
                strict: Some(false),
                schema: json!({ "type": "object" }),
            }),
        });

        let command = ChatCompletionCommand::from(request);

        assert!(matches!(
            command.structured_output,
            Some(StructuredOutput::JsonSchema(ref schema))
                if schema.name == "team_schema_v1"
                    && schema.description.as_deref() == Some("example schema")
                    && schema.strict == Some(false)
                    && schema.schema == json!({ "type": "object" })
        ));
    }

    #[test]
    fn stop_string_normalizes_to_vec() {
        let mut request = make_request();
        request.stop = Some(crate::api::v1::chat::schema::StopSequences::Single("END".to_owned()));

        let command = ChatCompletionCommand::from(request);

        assert_eq!(command.stop, vec!["END".to_owned()]);
    }

    #[test]
    fn completion_request_defaults_n_to_one() {
        let command = TextCompletionCommand::from(make_completion_request());

        assert_eq!(command.n, 1);
    }

    #[test]
    fn completion_response_format_maps_to_grammar_json() {
        let mut request = make_completion_request();
        request.response_format = Some(ChatResponseFormat {
            format_type: ChatResponseFormatType::JsonObject,
            schema: None,
            json_schema: None,
        });

        let command = TextCompletionCommand::from(request);

        assert!(command.grammar_json);
        assert_eq!(command.structured_output, Some(StructuredOutput::JsonObject));
    }

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
        assert!(super::serialize_session_message(&message).contains("<think status=\"done\">"));
    }
}
