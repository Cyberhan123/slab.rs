//! OpenAI-compatible API v1 request / response types.
//!
//! The structures here are intentionally kept compatible with the OpenAI REST
//! API specification so that existing OpenAI SDK clients work without
//! modification.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ── Chat Completions ─────────────────────────────────────────────────────────

/// A single message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatMessage {
    /// The role of the message author (`"system"`, `"user"`, `"assistant"`).
    pub role: String,
    /// The content of the message.
    pub content: String,
}

/// Request body for `POST /v1/chat/completions`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatCompletionRequest {
    /// Optional chat session ID for stateful conversations.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<String>,
    /// The model identifier to use (maps to a loaded slab-core backend).
    pub model: String,
    /// Conversation history; the last user message is used as the prompt.
    pub messages: Vec<ChatMessage>,
    /// When `true`, the response is streamed token-by-token using SSE.
    #[serde(default)]
    pub stream: bool,
    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Sampling temperature in [0, 2].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// A single choice in the completion response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatChoice {
    /// Zero-based index of this choice.
    pub index: u32,
    /// The generated message.
    pub message: ChatMessage,
    /// Why generation stopped (`"stop"`, `"length"`, …).
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
