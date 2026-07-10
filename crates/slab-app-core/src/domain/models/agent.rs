//! Agent application-layer command and response models.
//!
//! These types keep `/v1/agents/responses` use-case semantics out of the HTTP
//! transport while avoiding dependencies on axum, WebSocket, SSE, or OpenAPI
//! schema types.

use slab_agent::config::AgentConfig;
use slab_agent::port::{ThreadMessageRecord, ThreadSnapshot};
use slab_types::ConversationMessage;

/// Command accepted by the app-core agent application service.
#[derive(Debug)]
pub enum AgentCommand {
    // `config` is boxed so the enum isn't dominated by the large `AgentConfig`
    // variant (clippy::large_enum_variant).
    CreateResponse {
        session_id: String,
        config: Box<AgentConfig>,
        messages: Vec<ConversationMessage>,
    },
    AppendInput {
        thread_id: String,
        content: String,
    },
}

/// Transport-neutral session restoration payload.
#[derive(Debug)]
pub struct AgentSessionSnapshot {
    pub session_id: String,
    pub thread: Option<ThreadSnapshot>,
    pub messages: Vec<ThreadMessageRecord>,
    pub responses: Vec<serde_json::Value>,
}

/// Agent control-plane command accepted by the app-core agent application service.
#[derive(Debug)]
pub enum AgentControlCommand {
    ResolveApproval { thread_id: String, call_id: String, approved: bool },
    Interrupt { thread_id: String },
    Shutdown { thread_id: String },
}

/// Result returned after app-core applies an [`AgentControlCommand`].
#[derive(Debug, Clone)]
pub struct AgentControlResult {
    pub thread_id: String,
    pub delivered: Option<bool>,
    pub status: Option<AgentControlStatus>,
}

/// Transport-neutral thread status used by control acknowledgements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentControlStatus {
    Interrupting,
    Shutdown,
}
