//! OpenAI-compatible API v1 request / response types.
//!
//! The structures here are intentionally kept compatible with the OpenAI REST
//! API shape while also accepting richer structured content for future
//! multimodal and tool-calling work.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

use crate::domain::models::{
    ChatCompletionCommand as DomainChatCompletionCommand,
    ChatCompletionResult as DomainChatCompletionResult, ChatModelOption as DomainChatModelOption,
    ChatModelCapabilities as DomainChatModelCapabilities,
    ChatModelSource as DomainChatModelSource, ChatReasoningEffort as DomainChatReasoningEffort,
    ChatResultChoice as DomainChatResultChoice, ChatStreamOptions as DomainChatStreamOptions,
    ChatVerbosity as DomainChatVerbosity, CloudChatParams as DomainCloudChatParams,
    CommonChatParams as DomainCommonChatParams,
    ConversationContentPart as DomainConversationContentPart,
    ConversationMessage as DomainConversationMessage,
    ConversationMessageContent as DomainConversationMessageContent,
    ConversationToolCall as DomainConversationToolCall,
    ConversationToolFunction as DomainConversationToolFunction,
    LocalChatParams as DomainLocalChatParams,
    StructuredOutput as DomainStructuredOutput,
    StructuredOutputJsonSchema as DomainStructuredOutputJsonSchema,
    TextCompletionCommand as DomainTextCompletionCommand,
    TextCompletionResult as DomainTextCompletionResult, TextResultChoice as DomainTextResultChoice,
};
use slab_types::inference::TextGenerationUsage;

const MAX_PROMPT_BYTES: usize = 128 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatContentPart {
    Text {
        text: String,
    },
    InputText {
        text: String,
    },
    OutputText {
        text: String,
    },
    Image {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        image_url: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        mime_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        detail: Option<String>,
    },
    ToolResult {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        tool_call_id: Option<String>,
        value: serde_json::Value,
    },
    Json {
        value: serde_json::Value,
    },
    Refusal {
        text: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum ChatMessageContent {
    Text(String),
    Parts(Vec<ChatContentPart>),
}

impl Default for ChatMessageContent {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatToolFunction {
    pub name: String,
    #[serde(default)]
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatToolCall {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<String>,
    #[serde(default = "default_tool_call_type")]
    pub r#type: String,
    pub function: ChatToolFunction,
}

fn default_tool_call_type() -> String {
    "function".to_owned()
}

/// A single message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
#[validate(schema(function = "validate_chat_message"))]
pub struct ChatMessage {
    /// The role of the message author.
    pub role: String,
    /// String content or a richer structured content array.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub content: Option<ChatMessageContent>,
    /// Optional participant name for providers that support named turns.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    /// Tool call id for tool result messages.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tool_call_id: Option<String>,
    /// Assistant-emitted tool calls.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tool_calls: Vec<ChatToolCall>,
}

impl ChatMessage {
    pub fn has_meaningful_payload(&self) -> bool {
        self.content.as_ref().is_some_and(ChatMessageContent::has_meaningful_content)
            || !self.tool_calls.is_empty()
    }

    pub fn rendered_text(&self) -> String {
        DomainConversationMessage::from(self.clone()).rendered_text()
    }
}

impl ChatMessageContent {
    pub fn has_meaningful_content(&self) -> bool {
        match self {
            Self::Text(text) => !text.trim().is_empty(),
            Self::Parts(parts) => parts.iter().any(chat_content_part_has_meaningful_content),
        }
    }
}

fn chat_content_part_has_meaningful_content(part: &ChatContentPart) -> bool {
    match part {
        ChatContentPart::Text { text }
        | ChatContentPart::InputText { text }
        | ChatContentPart::OutputText { text }
        | ChatContentPart::Refusal { text } => !text.trim().is_empty(),
        ChatContentPart::Image { image_url, mime_type, .. } => {
            image_url.as_deref().is_some_and(|value| !value.trim().is_empty())
                || mime_type.as_deref().is_some_and(|value| !value.trim().is_empty())
        }
        ChatContentPart::ToolResult { .. } | ChatContentPart::Json { .. } => true,
    }
}

/// Reasoning effort hint for cloud chat providers that support thinking control.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChatReasoningEffort {
    None,
    Low,
    Medium,
    High,
    Minimal,
}

/// Verbosity hint for cloud chat providers that support thinking control.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChatVerbosity {
    Low,
    Medium,
    High,
}

/// High-level thinking toggle used by chat clients.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChatThinkingType {
    Enabled,
    Disabled,
}

/// Chat model source type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChatModelSource {
    Local,
    Cloud,
}

/// Thinking settings accepted by `POST /v1/chat/completions`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatThinkingConfig {
    /// Whether server-side reasoning should be enabled for this request.
    #[serde(rename = "type")]
    pub mode: ChatThinkingType,
    /// Optional reasoning effort override when `type = enabled`.
    #[serde(skip_serializing_if = "Option::is_none", default, alias = "reasoningEffort")]
    pub reasoning_effort: Option<ChatReasoningEffort>,
    /// Optional verbosity override when `type = enabled`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub verbosity: Option<ChatVerbosity>,
}

/// Streaming controls accepted by `POST /v1/chat/completions`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatStreamOptions {
    /// Whether the final chunk should include a usage payload.
    #[serde(default = "default_include_usage")]
    pub include_usage: bool,
}

fn default_include_usage() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum StopSequences {
    Single(String),
    Multiple(Vec<String>),
}

impl StopSequences {
    pub fn normalized(&self) -> Vec<String> {
        match self {
            Self::Single(value) => normalize_stop_values(std::iter::once(value.as_str())),
            Self::Multiple(values) => normalize_stop_values(values.iter().map(String::as_str)),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Single(value) => value.trim().is_empty(),
            Self::Multiple(values) => values.iter().all(|value| value.trim().is_empty()),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChatResponseFormatType {
    Text,
    JsonObject,
    JsonSchema,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatResponseJsonSchema {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub strict: Option<bool>,
    pub schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatResponseFormat {
    #[serde(rename = "type")]
    pub format_type: ChatResponseFormatType,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub json_schema: Option<ChatResponseJsonSchema>,
}

/// Request body for `POST /v1/chat/completions`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
#[validate(schema(function = "validate_chat_completion_request"))]
pub struct ChatCompletionRequest {
    /// Optional chat session ID for stateful conversations.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "id must not be empty"
    ))]
    pub id: Option<String>,
    /// Unified model identifier from `/v1/models`.
    /// `GET /v1/chat/models` returns picker options that reuse the same ids.
    #[serde(default)]
    pub model: String,
    /// Conversation history; the last user message is used as the prompt.
    #[validate(length(min = 1, message = "messages must not be empty"))]
    #[validate(nested)]
    pub messages: Vec<ChatMessage>,
    /// When `true`, continue generating from the last assistant message instead of starting a new turn.
    #[serde(default)]
    pub continue_generation: bool,
    /// When `true`, the response is streamed token-by-token using SSE.
    #[serde(default)]
    pub stream: bool,
    /// Controls streaming-only behavior.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub stream_options: Option<ChatStreamOptions>,
    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 1, max = 4096, message = "max_tokens must be between 1 and 4096"))]
    pub max_tokens: Option<u32>,
    /// Sampling temperature in [0, 2].
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, max = 2.0, message = "temperature must be between 0.0 and 2.0"))]
    pub temperature: Option<f32>,
    /// Nucleus sampling threshold in (0, 1].
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, max = 1.0, message = "top_p must be between 0.0 and 1.0"))]
    pub top_p: Option<f32>,
    /// Number of completions to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 1, message = "n must be at least 1"))]
    pub n: Option<u32>,
    /// Optional stop sequences.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub stop: Option<StopSequences>,
    /// Raw grammar passed through to the local llama backend.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub grammar: Option<String>,
    /// OpenAI-style structured output hint.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub response_format: Option<ChatResponseFormat>,
    /// Legacy llama.cpp-compatible top-level JSON schema field.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub json_schema: Option<Value>,
    /// Optional client-side thinking toggle. Accepted for compatibility with Ant Design X providers.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub thinking: Option<ChatThinkingConfig>,
    /// Optional provider reasoning effort override.
    #[serde(skip_serializing_if = "Option::is_none", default, alias = "reasoningEffort")]
    pub reasoning_effort: Option<ChatReasoningEffort>,
    /// Optional provider verbosity override.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub verbosity: Option<ChatVerbosity>,
}

/// Request body for `POST /v1/completions`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
#[validate(schema(function = "validate_completion_request"))]
pub struct CompletionRequest {
    /// Unified model identifier from `/v1/models`.
    /// When omitted, the first available chat-compatible model is used.
    #[serde(default)]
    pub model: String,
    /// Raw prompt for completion-style generation.
    pub prompt: String,
    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 1, max = 4096, message = "max_tokens must be between 1 and 4096"))]
    pub max_tokens: Option<u32>,
    /// Sampling temperature in [0, 2].
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, max = 2.0, message = "temperature must be between 0.0 and 2.0"))]
    pub temperature: Option<f32>,
    /// Nucleus sampling threshold in (0, 1].
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, max = 1.0, message = "top_p must be between 0.0 and 1.0"))]
    pub top_p: Option<f32>,
    /// Number of completions to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 1, message = "n must be at least 1"))]
    pub n: Option<u32>,
    /// Optional stop sequences.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub stop: Option<StopSequences>,
    /// Stream the result using SSE.
    #[serde(default)]
    pub stream: bool,
    /// Raw grammar passed through to the local llama backend.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub grammar: Option<String>,
    /// OpenAI-style structured output hint.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub response_format: Option<ChatResponseFormat>,
    /// Legacy llama.cpp-compatible top-level JSON schema field.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub json_schema: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatPromptTokensDetails {
    #[serde(default)]
    pub cached_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatCompletionUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub prompt_tokens_details: ChatPromptTokensDetails,
    #[serde(default)]
    pub estimated: bool,
}

/// A single choice in the completion response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatChoice {
    /// Zero-based index of this choice.
    pub index: u32,
    /// The generated message.
    pub message: ChatMessage,
    /// Why generation stopped (`"stop"`, `"length"`, ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

/// Response body for `POST /v1/chat/completions`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatCompletionResponse {
    /// Unique identifier for this completion.
    pub id: String,
    /// Always `"chat.completion"`.
    pub object: String,
    /// Unix timestamp of when the response was created.
    pub created: i64,
    /// Model that produced the completion.
    pub model: String,
    /// Backend/system fingerprint for compatibility with OpenAI clients.
    pub system_fingerprint: String,
    /// Generated choices.
    pub choices: Vec<ChatChoice>,
    /// Usage statistics for the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ChatCompletionUsage>,
}

/// A single choice in the text completion response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompletionChoice {
    /// Zero-based index of this choice.
    pub index: u32,
    /// Generated text for this choice.
    pub text: String,
    /// Why generation stopped (`"stop"`, `"length"`, ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

/// Response body for `POST /v1/completions`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompletionResponse {
    /// Unique identifier for this completion.
    pub id: String,
    /// Always `"text_completion"`.
    pub object: String,
    /// Unix timestamp of when the response was created.
    pub created: i64,
    /// Model that produced the completion.
    pub model: String,
    /// Backend/system fingerprint for compatibility with OpenAI clients.
    pub system_fingerprint: String,
    /// Generated choices.
    pub choices: Vec<CompletionChoice>,
    /// Usage statistics for the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ChatCompletionUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OpenAiError {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub param: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OpenAiErrorResponse {
    pub error: OpenAiError,
}

/// A selectable chat model option from `GET /v1/chat/models`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatModelOption {
    /// Stable option id used in `POST /v1/chat/completions`.
    pub id: String,
    /// User-facing display label.
    pub display_name: String,
    /// Whether this option is local or cloud.
    pub source: ChatModelSource,
    /// Whether model artifacts are already downloaded locally.
    pub downloaded: bool,
    /// Whether a model download task is running.
    pub pending: bool,
    /// Route-level feature flags for this model option.
    pub capabilities: ChatModelCapabilities,
    /// Backend id when `source = local`, e.g. `"ggml.llama"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_id: Option<String>,
    /// Cloud provider id when `source = cloud`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    /// Cloud provider name when `source = cloud`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatModelCapabilities {
    pub raw_grammar: bool,
    pub structured_output: bool,
    pub reasoning_controls: bool,
}

impl From<DomainConversationContentPart> for ChatContentPart {
    fn from(value: DomainConversationContentPart) -> Self {
        match value {
            DomainConversationContentPart::Text { text } => Self::Text { text },
            DomainConversationContentPart::InputText { text } => Self::InputText { text },
            DomainConversationContentPart::OutputText { text } => Self::OutputText { text },
            DomainConversationContentPart::Image { image_url, mime_type, detail } => {
                Self::Image { image_url, mime_type, detail }
            }
            DomainConversationContentPart::ToolResult { tool_call_id, value } => {
                Self::ToolResult { tool_call_id, value }
            }
            DomainConversationContentPart::Json { value } => Self::Json { value },
            DomainConversationContentPart::Refusal { text } => Self::Refusal { text },
        }
    }
}

impl From<ChatContentPart> for DomainConversationContentPart {
    fn from(value: ChatContentPart) -> Self {
        match value {
            ChatContentPart::Text { text } => DomainConversationContentPart::Text { text },
            ChatContentPart::InputText { text } => {
                DomainConversationContentPart::InputText { text }
            }
            ChatContentPart::OutputText { text } => {
                DomainConversationContentPart::OutputText { text }
            }
            ChatContentPart::Image { image_url, mime_type, detail } => {
                DomainConversationContentPart::Image { image_url, mime_type, detail }
            }
            ChatContentPart::ToolResult { tool_call_id, value } => {
                DomainConversationContentPart::ToolResult { tool_call_id, value }
            }
            ChatContentPart::Json { value } => DomainConversationContentPart::Json { value },
            ChatContentPart::Refusal { text } => DomainConversationContentPart::Refusal { text },
        }
    }
}

impl From<DomainConversationMessageContent> for ChatMessageContent {
    fn from(value: DomainConversationMessageContent) -> Self {
        match value {
            DomainConversationMessageContent::Text(text) => Self::Text(text),
            DomainConversationMessageContent::Parts(parts) => {
                Self::Parts(parts.into_iter().map(Into::into).collect())
            }
        }
    }
}

impl From<ChatMessageContent> for DomainConversationMessageContent {
    fn from(value: ChatMessageContent) -> Self {
        match value {
            ChatMessageContent::Text(text) => Self::Text(text),
            ChatMessageContent::Parts(parts) => {
                Self::Parts(parts.into_iter().map(Into::into).collect())
            }
        }
    }
}

impl From<DomainConversationToolFunction> for ChatToolFunction {
    fn from(value: DomainConversationToolFunction) -> Self {
        Self { name: value.name, arguments: value.arguments }
    }
}

impl From<ChatToolFunction> for DomainConversationToolFunction {
    fn from(value: ChatToolFunction) -> Self {
        Self { name: value.name, arguments: value.arguments }
    }
}

impl From<DomainConversationToolCall> for ChatToolCall {
    fn from(value: DomainConversationToolCall) -> Self {
        Self { id: value.id, r#type: value.r#type, function: value.function.into() }
    }
}

impl From<ChatToolCall> for DomainConversationToolCall {
    fn from(value: ChatToolCall) -> Self {
        Self { id: value.id, r#type: value.r#type, function: value.function.into() }
    }
}

impl From<ChatReasoningEffort> for DomainChatReasoningEffort {
    fn from(value: ChatReasoningEffort) -> Self {
        match value {
            ChatReasoningEffort::None => Self::None,
            ChatReasoningEffort::Low => Self::Low,
            ChatReasoningEffort::Medium => Self::Medium,
            ChatReasoningEffort::High => Self::High,
            ChatReasoningEffort::Minimal => Self::Minimal,
        }
    }
}

impl From<ChatVerbosity> for DomainChatVerbosity {
    fn from(value: ChatVerbosity) -> Self {
        match value {
            ChatVerbosity::Low => Self::Low,
            ChatVerbosity::Medium => Self::Medium,
            ChatVerbosity::High => Self::High,
        }
    }
}

impl From<ChatStreamOptions> for DomainChatStreamOptions {
    fn from(value: ChatStreamOptions) -> Self {
        Self { include_usage: value.include_usage }
    }
}

impl From<ChatCompletionRequest> for DomainChatCompletionCommand {
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
        let stop = stop.as_ref().map(StopSequences::normalized).unwrap_or_default();

        Self {
            id,
            model: model.trim().to_owned(),
            messages: messages.into_iter().map(Into::into).collect(),
            continue_generation,
            common: DomainCommonChatParams {
                max_tokens,
                temperature,
                top_p,
                n: n.unwrap_or(1),
                stream,
                stop,
                stream_options: stream_options.map(Into::into).unwrap_or_default(),
            },
            local: DomainLocalChatParams {
                grammar,
                structured_output: structured_output.clone(),
            },
            cloud: DomainCloudChatParams {
                reasoning_effort,
                verbosity,
                structured_output,
            },
        }
    }
}

impl From<CompletionRequest> for DomainTextCompletionCommand {
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
        let stop = stop.as_ref().map(StopSequences::normalized).unwrap_or_default();

        Self {
            model: model.trim().to_owned(),
            prompt,
            common: DomainCommonChatParams {
                max_tokens,
                temperature,
                top_p,
                n: n.unwrap_or(1),
                stream,
                stop,
                stream_options: DomainChatStreamOptions::default(),
            },
            local: DomainLocalChatParams {
                grammar,
                structured_output: structured_output.clone(),
            },
            cloud: DomainCloudChatParams {
                reasoning_effort: None,
                verbosity: None,
                structured_output,
            },
        }
    }
}

impl From<DomainConversationMessage> for ChatMessage {
    fn from(message: DomainConversationMessage) -> Self {
        Self {
            role: message.role,
            content: Some(message.content.into()),
            name: message.name,
            tool_call_id: message.tool_call_id,
            tool_calls: message.tool_calls.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ChatMessage> for DomainConversationMessage {
    fn from(message: ChatMessage) -> Self {
        Self {
            role: message.role,
            content: message.content.unwrap_or_default().into(),
            name: message.name,
            tool_call_id: message.tool_call_id,
            tool_calls: message.tool_calls.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<DomainChatResultChoice> for ChatChoice {
    fn from(choice: DomainChatResultChoice) -> Self {
        Self {
            index: choice.index,
            message: choice.message.into(),
            finish_reason: choice.finish_reason,
        }
    }
}

impl From<DomainTextResultChoice> for CompletionChoice {
    fn from(choice: DomainTextResultChoice) -> Self {
        Self { index: choice.index, text: choice.text, finish_reason: choice.finish_reason }
    }
}

impl From<TextGenerationUsage> for ChatCompletionUsage {
    fn from(value: TextGenerationUsage) -> Self {
        Self {
            prompt_tokens: value.prompt_tokens,
            completion_tokens: value.completion_tokens,
            total_tokens: value.total_tokens,
            prompt_tokens_details: ChatPromptTokensDetails {
                cached_tokens: value.prompt_tokens_details.cached_tokens,
            },
            estimated: value.estimated,
        }
    }
}

impl From<DomainChatCompletionResult> for ChatCompletionResponse {
    fn from(result: DomainChatCompletionResult) -> Self {
        Self {
            id: result.id,
            object: result.object,
            created: result.created,
            model: result.model,
            system_fingerprint: result.system_fingerprint,
            choices: result.choices.into_iter().map(Into::into).collect(),
            usage: result.usage.map(Into::into),
        }
    }
}

impl From<DomainTextCompletionResult> for CompletionResponse {
    fn from(result: DomainTextCompletionResult) -> Self {
        Self {
            id: result.id,
            object: result.object,
            created: result.created,
            model: result.model,
            system_fingerprint: result.system_fingerprint,
            choices: result.choices.into_iter().map(Into::into).collect(),
            usage: result.usage.map(Into::into),
        }
    }
}

impl From<DomainChatModelSource> for ChatModelSource {
    fn from(value: DomainChatModelSource) -> Self {
        match value {
            DomainChatModelSource::Local => Self::Local,
            DomainChatModelSource::Cloud => Self::Cloud,
        }
    }
}

impl From<DomainChatModelCapabilities> for ChatModelCapabilities {
    fn from(value: DomainChatModelCapabilities) -> Self {
        Self {
            raw_grammar: value.raw_grammar,
            structured_output: value.structured_output,
            reasoning_controls: value.reasoning_controls,
        }
    }
}

impl From<DomainChatModelOption> for ChatModelOption {
    fn from(value: DomainChatModelOption) -> Self {
        Self {
            id: value.id,
            display_name: value.display_name,
            source: value.source.into(),
            downloaded: value.downloaded,
            pending: value.pending,
            capabilities: value.capabilities.into(),
            backend_id: value.backend_id,
            provider_id: value.provider_id,
            provider_name: value.provider_name,
        }
    }
}

fn validate_chat_message(message: &ChatMessage) -> Result<(), ValidationError> {
    crate::api::validation::validate_chat_role(&message.role)?;
    if !message.has_meaningful_payload() {
        let mut error = ValidationError::new("blank_message");
        error.message = Some("content must not be empty".into());
        return Err(error);
    }
    Ok(())
}

fn validate_chat_completion_request(
    request: &ChatCompletionRequest,
) -> Result<(), ValidationError> {
    if request.stream && request.n.unwrap_or(1) > 1 {
        return Err(validation_error(
            "unsupported_combination",
            "streaming with n > 1 is not supported",
        ));
    }
    if request.stream && request.stop.as_ref().is_some_and(|stop| !stop.is_empty()) {
        return Err(validation_error(
            "unsupported_combination",
            "streaming with stop is not supported for chat completions",
        ));
    }
    validate_structured_output(request.response_format.as_ref(), request.json_schema.as_ref())?;

    let Some(user_message) = request
        .messages
        .iter()
        .rev()
        .find(|message| message.role == "user" && message.has_meaningful_payload())
    else {
        let mut error = ValidationError::new("no_user_message");
        error.message = Some("messages must contain at least one user message".into());
        return Err(error);
    };

    let rendered = user_message.rendered_text();
    if !rendered.is_empty() && rendered.len() > MAX_PROMPT_BYTES {
        let mut error = ValidationError::new("prompt_too_large");
        error.message = Some(
            format!(
                "last user message is too large ({} bytes); maximum is {} bytes",
                rendered.len(),
                MAX_PROMPT_BYTES
            )
            .into(),
        );
        return Err(error);
    }

    if request.continue_generation {
        let Some(last_message) =
            request.messages.iter().rev().find(|message| message.has_meaningful_payload())
        else {
            return Err(validation_error(
                "invalid_continue_generation",
                "continue_generation requires a non-empty assistant message",
            ));
        };

        if last_message.role != "assistant" || last_message.rendered_text().trim().is_empty() {
            return Err(validation_error(
                "invalid_continue_generation",
                "continue_generation requires the last meaningful message to be a non-empty assistant message",
            ));
        }
    }

    Ok(())
}

fn validate_completion_request(request: &CompletionRequest) -> Result<(), ValidationError> {
    if request.stream && request.n.unwrap_or(1) > 1 {
        return Err(validation_error(
            "unsupported_combination",
            "streaming with n > 1 is not supported",
        ));
    }
    validate_structured_output(request.response_format.as_ref(), request.json_schema.as_ref())?;

    let prompt = request.prompt.trim();
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err(validation_error(
            "prompt_too_large",
            &format!(
                "prompt is too large ({} bytes); maximum is {} bytes",
                prompt.len(),
                MAX_PROMPT_BYTES
            ),
        ));
    }

    Ok(())
}

fn validate_structured_output(
    response_format: Option<&ChatResponseFormat>,
    json_schema: Option<&Value>,
) -> Result<(), ValidationError> {
    if let Some(schema) = json_schema {
        validate_schema_like("json_schema", schema)?;
    }

    let Some(response_format) = response_format else {
        return Ok(());
    };

    if let Some(schema) = response_format.schema.as_ref() {
        validate_schema_like("response_format.schema", schema)?;
    }

    if let Some(json_schema) = response_format.json_schema.as_ref() {
        validate_schema_like("response_format.json_schema.schema", &json_schema.schema)?;
    }

    Ok(())
}

fn validate_schema_like(field: &str, schema: &Value) -> Result<(), ValidationError> {
    match schema {
        Value::Bool(_) => Ok(()),
        Value::Object(object) => {
            if let Some(value) = object.get("type") {
                validate_json_schema_type(field, value)?;
            }
            Ok(())
        }
        _ => Err(validation_error(
            "invalid_schema",
            &format!("{field} must be a JSON object or boolean"),
        )),
    }
}

fn validate_json_schema_type(field: &str, value: &Value) -> Result<(), ValidationError> {
    let allowed = ["string", "number", "integer", "boolean", "object", "array", "null"];

    match value {
        Value::String(kind) if allowed.contains(&kind.as_str()) => Ok(()),
        Value::Array(items)
            if items
                .iter()
                .all(|item| item.as_str().is_some_and(|kind| allowed.contains(&kind))) =>
        {
            Ok(())
        }
        _ => Err(validation_error(
            "invalid_schema",
            &format!("{field} type must be a valid JSON Schema type"),
        )),
    }
}

fn validation_error(code: &'static str, message: &str) -> ValidationError {
    let mut error = ValidationError::new(code);
    error.message = Some(message.to_owned().into());
    error
}

fn reasoning_effort_from_thinking(
    thinking: &ChatThinkingConfig,
) -> Option<DomainChatReasoningEffort> {
    match thinking.mode {
        ChatThinkingType::Disabled => Some(DomainChatReasoningEffort::None),
        ChatThinkingType::Enabled => {
            thinking.reasoning_effort.map(Into::into).or(Some(DomainChatReasoningEffort::Medium))
        }
    }
}

fn verbosity_from_thinking(thinking: &ChatThinkingConfig) -> Option<DomainChatVerbosity> {
    match thinking.mode {
        ChatThinkingType::Disabled => None,
        ChatThinkingType::Enabled => thinking.verbosity.map(Into::into),
    }
}

fn structured_output_from_api(
    response_format: Option<ChatResponseFormat>,
    json_schema: Option<Value>,
) -> Option<DomainStructuredOutput> {
    if let Some(schema) = json_schema {
        return Some(DomainStructuredOutput::JsonSchema(DomainStructuredOutputJsonSchema::new(
            None, None, None, schema,
        )));
    }

    let response_format = response_format?;
    match response_format.format_type {
        ChatResponseFormatType::Text => None,
        ChatResponseFormatType::JsonObject | ChatResponseFormatType::JsonSchema => response_format
            .json_schema
            .map(structured_output_json_schema_from_api)
            .map(DomainStructuredOutput::JsonSchema)
            .or_else(|| {
                response_format.schema.map(|schema| {
                    DomainStructuredOutput::JsonSchema(DomainStructuredOutputJsonSchema::new(
                        None, None, None, schema,
                    ))
                })
            })
            .or(Some(DomainStructuredOutput::JsonObject)),
    }
}

fn structured_output_json_schema_from_api(
    value: ChatResponseJsonSchema,
) -> DomainStructuredOutputJsonSchema {
    DomainStructuredOutputJsonSchema::new(value.name, value.description, value.strict, value.schema)
}

fn normalize_stop_values<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    values.into_iter().map(str::trim).filter(|value| !value.is_empty()).map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        ChatCompletionRequest, ChatMessage, ChatMessageContent, ChatReasoningEffort,
        ChatResponseFormat, ChatResponseFormatType, ChatResponseJsonSchema, ChatStreamOptions,
        ChatThinkingConfig, ChatThinkingType, ChatVerbosity, CompletionRequest, StopSequences,
    };
    use crate::domain::models::{
        ChatCompletionCommand as DomainChatCompletionCommand,
        ChatReasoningEffort as DomainChatReasoningEffort, ChatVerbosity as DomainChatVerbosity,
        StructuredOutput as DomainStructuredOutput,
        TextCompletionCommand as DomainTextCompletionCommand,
    };
    use serde_json::json;

    fn make_request() -> ChatCompletionRequest {
        ChatCompletionRequest {
            id: None,
            model: "cloud/provider/model".to_owned(),
            messages: vec![ChatMessage {
                role: "user".to_owned(),
                content: Some(ChatMessageContent::Text("hello".to_owned())),
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

        let command = DomainChatCompletionCommand::from(request);

        assert!(matches!(
            command.cloud.reasoning_effort,
            Some(DomainChatReasoningEffort::None)
        ));
    }

    #[test]
    fn thinking_enabled_defaults_to_medium_reasoning() {
        let mut request = make_request();
        request.thinking = Some(ChatThinkingConfig {
            mode: ChatThinkingType::Enabled,
            reasoning_effort: None,
            verbosity: None,
        });

        let command = DomainChatCompletionCommand::from(request);

        assert!(matches!(
            command.cloud.reasoning_effort,
            Some(DomainChatReasoningEffort::Medium)
        ));
    }

    #[test]
    fn explicit_reasoning_and_verbosity_take_precedence() {
        let mut request = make_request();
        request.thinking = Some(ChatThinkingConfig {
            mode: ChatThinkingType::Disabled,
            reasoning_effort: None,
            verbosity: None,
        });
        request.reasoning_effort = Some(ChatReasoningEffort::High);
        request.verbosity = Some(ChatVerbosity::Low);

        let command = DomainChatCompletionCommand::from(request);

        assert!(matches!(
            command.cloud.reasoning_effort,
            Some(DomainChatReasoningEffort::High)
        ));
        assert!(matches!(command.cloud.verbosity, Some(DomainChatVerbosity::Low)));
    }

    #[test]
    fn continue_generation_flag_is_preserved() {
        let mut request = make_request();
        request.continue_generation = true;

        let command = DomainChatCompletionCommand::from(request);

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

        let command = DomainChatCompletionCommand::from(request);

        assert_eq!(command.local.grammar, None);
        assert_eq!(command.local.structured_output, Some(DomainStructuredOutput::JsonObject));
        assert_eq!(command.cloud.structured_output, Some(DomainStructuredOutput::JsonObject));
    }

    #[test]
    fn response_format_schema_maps_to_structured_json_schema() {
        let mut request = make_request();
        request.response_format = Some(ChatResponseFormat {
            format_type: ChatResponseFormatType::JsonObject,
            schema: Some(json!({ "const": "42" })),
            json_schema: None,
        });

        let command = DomainChatCompletionCommand::from(request);

        assert!(matches!(
            command.local.structured_output,
            Some(DomainStructuredOutput::JsonSchema(ref schema))
                if schema.schema == json!({ "const": "42" })
        ));
    }

    #[test]
    fn json_schema_field_maps_to_grammar_json() {
        let mut request = make_request();
        request.json_schema = Some(json!({ "const": "42" }));

        let command = DomainChatCompletionCommand::from(request);

        assert!(matches!(
            command.local.structured_output,
            Some(DomainStructuredOutput::JsonSchema(ref schema))
                if schema.name == "slab_structured_output"
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

        let command = DomainChatCompletionCommand::from(request);

        assert!(matches!(
            command.cloud.structured_output,
            Some(DomainStructuredOutput::JsonSchema(ref schema))
                if schema.name == "team_schema_v1"
                    && schema.description.as_deref() == Some("example schema")
                    && schema.strict == Some(false)
                    && schema.schema == json!({ "type": "object" })
        ));
    }

    #[test]
    fn stop_string_normalizes_to_vec() {
        let mut request = make_request();
        request.stop = Some(StopSequences::Single("END".to_owned()));

        let command = DomainChatCompletionCommand::from(request);

        assert_eq!(command.common.stop, vec!["END".to_owned()]);
    }

    #[test]
    fn completion_request_defaults_n_to_one() {
        let command = DomainTextCompletionCommand::from(make_completion_request());

        assert_eq!(command.common.n, 1);
    }

    #[test]
    fn completion_response_format_maps_to_grammar_json() {
        let mut request = make_completion_request();
        request.response_format = Some(ChatResponseFormat {
            format_type: ChatResponseFormatType::JsonObject,
            schema: None,
            json_schema: None,
        });

        let command = DomainTextCompletionCommand::from(request);

        assert_eq!(command.local.structured_output, Some(DomainStructuredOutput::JsonObject));
        assert_eq!(command.cloud.structured_output, Some(DomainStructuredOutput::JsonObject));
    }
}
