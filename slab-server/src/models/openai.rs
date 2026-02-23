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
    /// Optional chat session ID for stateful conversations.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub session_id: Option<String>,
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

// ── Audio Transcriptions ─────────────────────────────────────────────────────

/// Response body for `POST /v1/audio/transcriptions`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TranscriptionResponse {
    /// Transcribed text.
    pub text: String,
}

// ── Image Generations ────────────────────────────────────────────────────────

/// Request body for `POST /v1/images/generations`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ImageGenerationRequest {
    /// The model identifier to use.
    pub model: String,
    /// Text description of the desired image.
    pub prompt: String,
    /// Number of images to generate (default `1`).
    #[serde(default = "default_n")]
    pub n: u32,
    /// Desired image size, e.g. `"512x512"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
}

fn default_n() -> u32 {
    1
}

/// A single generated image returned as base64-encoded PNG.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ImageData {
    /// Base64-encoded image data.
    pub b64_json: String,
}

/// Response body for `POST /v1/images/generations`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ImageGenerationResponse {
    /// Unix timestamp of when the response was created.
    pub created: i64,
    /// Generated images.
    pub data: Vec<ImageData>,
}

// ── Models list ──────────────────────────────────────────────────────────────

/// A single model descriptor (OpenAI `/v1/models` format).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModelInfo {
    /// Model identifier string.
    pub id: String,
    /// Always `"model"`.
    pub object: String,
    /// Unix timestamp of when the model was created / loaded.
    pub created: i64,
    /// Owner of the model.
    pub owned_by: String,
}

/// Response body for `GET /v1/models`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModelListResponse {
    /// Always `"list"`.
    pub object: String,
    /// Available models.
    pub data: Vec<ModelInfo>,
}
