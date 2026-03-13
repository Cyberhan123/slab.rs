//! OpenAI-compatible API v1 request / response types.
//!
//! The structures here are intentionally kept compatible with the OpenAI REST
//! API specification so that existing OpenAI SDK clients work without
//! modification.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

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
    /// The model identifier to use. It can be:
    /// - local model id from `/v1/models`
    /// - cloud model option id from `GET /v1/chat/models`
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
    #[validate(range(
        min = 0.0,
        max = 2.0,
        message = "temperature must be between 0.0 and 2.0"
    ))]
    pub temperature: Option<f32>,
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

/// Chat model source type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChatModelSource {
    Local,
    Cloud,
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
    /// Cloud provider id when `source = cloud`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub provider_id: Option<String>,
    /// Cloud provider name when `source = cloud`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub provider_name: Option<String>,
    /// Backend id when `source = local`, e.g. `ggml.llama`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub backend_id: Option<String>,
    /// Whether model artifacts are already downloaded locally.
    pub downloaded: bool,
    /// Whether a model download task is running.
    pub pending: bool,
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
