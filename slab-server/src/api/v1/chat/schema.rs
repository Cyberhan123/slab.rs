//! OpenAI-compatible API v1 request / response types.
//!
//! The structures here are intentionally kept compatible with the OpenAI REST
//! API specification so that existing OpenAI SDK clients work without
//! modification.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

use crate::domain::models::{
    ChatCompletionResult as DomainChatCompletionResult, ChatModelOption as DomainChatModelOption,
    ChatModelSource as DomainChatModelSource, ChatResultChoice as DomainChatResultChoice,
    ConversationMessage,
};

const MAX_PROMPT_BYTES: usize = 128 * 1024;

/// A single message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct ChatMessage {
    /// The role of the message author (`"system"`, `"user"`, `"assistant"`).
    #[validate(custom(
        function = "crate::api::validation::validate_chat_role",
        message = "role must be one of: system, user, assistant"
    ))]
    pub role: String,
    /// The content of the message.
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "content must not be empty"
    ))]
    pub content: String,
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
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "model must not be empty"
    ))]
    pub model: String,
    /// Conversation history; the last user message is used as the prompt.
    #[validate(length(min = 1, message = "messages must not be empty"))]
    #[validate(nested)]
    pub messages: Vec<ChatMessage>,
    /// When `true`, the response is streamed token-by-token using SSE.
    #[serde(default)]
    pub stream: bool,
    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 1, max = 4096, message = "max_tokens must be between 1 and 4096"))]
    pub max_tokens: Option<u32>,
    /// Sampling temperature in [0, 2].
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, max = 2.0, message = "temperature must be between 0.0 and 2.0"))]
    pub temperature: Option<f32>,
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

/// A single choice in the completion response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatChoice {
    /// Zero-based index of this choice.
    pub index: u32,
    /// The generated message.
    pub message: ChatMessage,
    /// Why generation stopped (`"stop"`, `"length"`, 鈥?.
    pub finish_reason: String,
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
    /// Generated choices.
    pub choices: Vec<ChatChoice>,
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

impl From<ConversationMessage> for ChatMessage {
    fn from(message: ConversationMessage) -> Self {
        Self { role: message.role, content: message.content }
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

impl From<DomainChatCompletionResult> for ChatCompletionResponse {
    fn from(result: DomainChatCompletionResult) -> Self {
        Self {
            id: result.id,
            object: result.object,
            created: result.created,
            model: result.model,
            choices: result.choices.into_iter().map(Into::into).collect(),
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

fn validate_chat_completion_request(
    request: &ChatCompletionRequest,
) -> Result<(), ValidationError> {
    let Some(user_message) = request
        .messages
        .iter()
        .rev()
        .find(|message| message.role == "user" && !message.content.trim().is_empty())
    else {
        let mut error = ValidationError::new("no_user_message");
        error.message = Some("messages must contain at least one user message".into());
        return Err(error);
    };

    if user_message.content.len() > MAX_PROMPT_BYTES {
        let mut error = ValidationError::new("prompt_too_large");
        error.message = Some(
            format!(
                "last user message is too large ({} bytes); maximum is {} bytes",
                user_message.content.len(),
                MAX_PROMPT_BYTES
            )
            .into(),
        );
        return Err(error);
    }

    Ok(())
}
