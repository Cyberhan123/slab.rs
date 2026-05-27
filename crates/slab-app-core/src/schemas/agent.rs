//! Request and response schemas for the `/v1/agents/*` routes.

use serde::{Deserialize, Serialize};
use slab_agent::config::AgentConfig;
use slab_agent::port::{ThreadMessageRecord, ThreadSnapshot};
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

/// Persisted agent thread summary.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AgentThreadResponse {
    pub id: String,
    pub session_id: String,
    pub parent_id: Option<String>,
    pub depth: u32,
    pub status: AgentStatusValue,
    pub role_name: Option<String>,
    pub completion_text: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Persisted agent thread message.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AgentThreadMessageResponse {
    pub id: String,
    pub thread_id: String,
    pub turn_index: u32,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

impl From<ThreadSnapshot> for AgentThreadResponse {
    fn from(thread: ThreadSnapshot) -> Self {
        Self {
            id: thread.id,
            session_id: thread.session_id,
            parent_id: thread.parent_id,
            depth: thread.depth,
            status: thread.status.into(),
            role_name: thread.role_name,
            completion_text: thread.completion_text,
            created_at: thread.created_at,
            updated_at: thread.updated_at,
        }
    }
}

impl From<ThreadMessageRecord> for AgentThreadMessageResponse {
    fn from(record: ThreadMessageRecord) -> Self {
        let message = record.message;
        let content = message.rendered_text();
        Self {
            id: record.id,
            thread_id: record.thread_id,
            turn_index: record.turn_index,
            role: message.role,
            content,
            created_at: record.created_at,
        }
    }
}

// ── Input ─────────────────────────────────────────────────────────────────────

/// Request body for `POST /v1/agents/{id}/input`.
#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct AgentInputRequest {
    /// Plain-text message to append to the agent thread's conversation.
    #[validate(custom(function = "validate_non_blank", message = "content must not be blank"))]
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
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatusValue {
    Pending,
    Running,
    Interrupting,
    Interrupted,
    Completed,
    Errored,
    Shutdown,
}

impl From<AgentThreadStatus> for AgentStatusValue {
    fn from(s: AgentThreadStatus) -> Self {
        match s {
            AgentThreadStatus::Pending => Self::Pending,
            AgentThreadStatus::Running => Self::Running,
            AgentThreadStatus::Interrupting => Self::Interrupting,
            AgentThreadStatus::Interrupted => Self::Interrupted,
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

// ── Approve ───────────────────────────────────────────────────────────────────

/// Request body for `POST /v1/agents/{id}/approve`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AgentApproveRequest {
    /// The call ID of the pending tool call.
    pub call_id: String,
    /// `true` to approve the call, `false` to reject it.
    pub approved: bool,
}

/// Response body for `POST /v1/agents/{id}/approve`.
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentApproveResponse {
    pub call_id: String,
    pub delivered: bool,
}

/// Response body for `POST /v1/agents/{id}/interrupt`.
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentInterruptResponse {
    pub thread_id: String,
    pub interrupted: bool,
}
