//! Request and response schemas for the `/v1/agents/*` routes.

use serde::{Deserialize, Serialize};
use slab_agent::config::AgentConfig;
use slab_types::agent::AgentThreadStatus;
use utoipa::ToSchema;
use validator::Validate;

use crate::schemas::validation::validate_non_blank;

// ── Spawn ─────────────────────────────────────────────────────────────────────

/// Request body for `POST /v1/agents/spawn`.
#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct SpawnAgentRequest {
    /// Chat session ID that backs this agent thread.
    #[validate(custom(function = "validate_non_blank", message = "session_id must not be blank"))]
    pub session_id: String,
    /// Agent runtime configuration (model, temperature, etc.).
    #[serde(default)]
    pub config: AgentConfigInput,
    /// Initial messages to seed the agent's conversation.
    #[serde(default)]
    #[validate(nested)]
    pub messages: Vec<MessageInput>,
}

/// Agent configuration provided by the caller.
#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct AgentConfigInput {
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: Option<u32>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub allowed_tools: Option<Vec<String>>,
}

impl From<AgentConfigInput> for AgentConfig {
    fn from(v: AgentConfigInput) -> Self {
        let defaults = AgentConfig::default();
        Self {
            model: v.model.unwrap_or(defaults.model),
            system_prompt: v.system_prompt,
            max_turns: v.max_turns.unwrap_or(defaults.max_turns),
            max_depth: defaults.max_depth,
            max_threads: defaults.max_threads,
            temperature: v.temperature.unwrap_or(defaults.temperature),
            max_tokens: v.max_tokens.unwrap_or(defaults.max_tokens),
            allowed_tools: v.allowed_tools.unwrap_or_default(),
        }
    }
}

/// A single message in the initial conversation.
#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct MessageInput {
    #[validate(custom(function = "validate_non_blank", message = "role must not be blank"))]
    pub role: String,
    pub content: String,
}

impl From<MessageInput> for slab_types::ConversationMessage {
    fn from(v: MessageInput) -> Self {
        slab_types::ConversationMessage {
            role: v.role,
            content: slab_types::ConversationMessageContent::Text(v.content),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }
    }
}

/// Response body for `POST /v1/agents/spawn`.
#[derive(Debug, Serialize, ToSchema)]
pub struct SpawnAgentResponse {
    /// Unique ID of the newly created agent thread.
    pub thread_id: String,
}

// ── Input ─────────────────────────────────────────────────────────────────────

/// Request body for `POST /v1/agents/{id}/input`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AgentInputRequest {
    /// Plain-text message to append to the agent thread's conversation.
    pub content: String,
}

/// Response body for `POST /v1/agents/{id}/input`.
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentInputResponse {
    /// `true` if the input was accepted.
    pub accepted: bool,
    pub message: String,
}

// ── Status ────────────────────────────────────────────────────────────────────

/// Response body for `GET /v1/agents/{id}/status`.
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentStatusResponse {
    pub thread_id: String,
    pub status: AgentStatusValue,
}

/// Serialisable mirror of [`AgentThreadStatus`].
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatusValue {
    Pending,
    Running,
    Completed,
    Errored,
    Shutdown,
}

impl From<AgentThreadStatus> for AgentStatusValue {
    fn from(s: AgentThreadStatus) -> Self {
        match s {
            AgentThreadStatus::Pending => Self::Pending,
            AgentThreadStatus::Running => Self::Running,
            AgentThreadStatus::Completed => Self::Completed,
            AgentThreadStatus::Errored => Self::Errored,
            AgentThreadStatus::Shutdown => Self::Shutdown,
        }
    }
}

// ── Shutdown ──────────────────────────────────────────────────────────────────

/// Response body for `POST /v1/agents/{id}/shutdown`.
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentShutdownResponse {
    pub thread_id: String,
    pub shutdown: bool,
}
