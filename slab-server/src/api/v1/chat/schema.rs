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
    ChatCompletionResult as DomainChatCompletionResult, ChatModelOption as DomainChatModelOption,
    ChatModelSource as DomainChatModelSource, ChatResultChoice as DomainChatResultChoice,
    ConversationContentPart as DomainConversationContentPart,
    ConversationMessage as DomainConversationMessage,
    ConversationMessageContent as DomainConversationMessageContent,
    ConversationToolCall as DomainConversationToolCall,
    ConversationToolFunction as DomainConversationToolFunction,
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

impl From<DomainChatModelOption> for ChatModelOption {
    fn from(value: DomainChatModelOption) -> Self {
        Self {
            id: value.id,
            display_name: value.display_name,
            source: value.source.into(),
            downloaded: value.downloaded,
            pending: value.pending,
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

fn normalize_stop_values<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    values.into_iter().map(str::trim).filter(|value| !value.is_empty()).map(str::to_owned).collect()
}
