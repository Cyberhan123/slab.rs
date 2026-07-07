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
    RestoreSession {
        request_id: Option<String>,
        session_id: String,
    },
    CreateResponse {
        request_id: Option<String>,
        session_id: String,
        config: AgentConfig,
        messages: Vec<ConversationMessage>,
    },
    AppendInput {
        request_id: Option<String>,
        thread_id: String,
        content: String,
    },
    ResolveApproval {
        request_id: Option<String>,
        thread_id: String,
        call_id: String,
        approved: bool,
    },
    Interrupt {
        request_id: Option<String>,
        thread_id: String,
    },
    Shutdown {
        request_id: Option<String>,
        thread_id: String,
    },
}

impl AgentCommand {
    /// Caller-provided request identifier to echo through transport responses.
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::RestoreSession { request_id, .. }
            | Self::CreateResponse { request_id, .. }
            | Self::AppendInput { request_id, .. }
            | Self::ResolveApproval { request_id, .. }
            | Self::Interrupt { request_id, .. }
            | Self::Shutdown { request_id, .. } => request_id.as_deref(),
        }
    }
}

/// Stable app-core action marker for accepted agent commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentCommandAction {
    RestoreSession,
    CreateResponse,
    AppendInput,
    ResolveApproval,
    Interrupt,
    Shutdown,
}

/// Result returned after app-core has applied an [`AgentCommand`].
#[derive(Debug)]
pub struct AgentCommandResult {
    pub request_id: Option<String>,
    pub action: AgentCommandAction,
    pub accepted: bool,
    pub thread_id: Option<String>,
    pub status: Option<AgentCommandStatus>,
    pub delivered: Option<bool>,
    pub session: Option<AgentSessionSnapshot>,
    pub subscribe_thread_id: Option<String>,
}

impl AgentCommandResult {
    pub fn ack(
        command: &AgentCommand,
        thread_id: Option<String>,
        status: Option<AgentCommandStatus>,
        delivered: Option<bool>,
    ) -> Self {
        Self {
            request_id: command.request_id().map(str::to_owned),
            action: command.action(),
            accepted: delivered.unwrap_or(true),
            subscribe_thread_id: thread_id.clone(),
            thread_id,
            status,
            delivered,
            session: None,
        }
    }

    pub fn restored(command: &AgentCommand, session: AgentSessionSnapshot) -> Self {
        let subscribe_thread_id = session.thread.as_ref().map(|thread| thread.id.clone());
        Self {
            request_id: command.request_id().map(str::to_owned),
            action: AgentCommandAction::RestoreSession,
            accepted: true,
            thread_id: subscribe_thread_id.clone(),
            status: None,
            delivered: None,
            session: Some(session),
            subscribe_thread_id,
        }
    }
}

impl AgentCommand {
    pub fn action(&self) -> AgentCommandAction {
        match self {
            Self::RestoreSession { .. } => AgentCommandAction::RestoreSession,
            Self::CreateResponse { .. } => AgentCommandAction::CreateResponse,
            Self::AppendInput { .. } => AgentCommandAction::AppendInput,
            Self::ResolveApproval { .. } => AgentCommandAction::ResolveApproval,
            Self::Interrupt { .. } => AgentCommandAction::Interrupt,
            Self::Shutdown { .. } => AgentCommandAction::Shutdown,
        }
    }
}

/// Transport-neutral session restoration payload.
#[derive(Debug)]
pub struct AgentSessionSnapshot {
    pub session_id: String,
    pub thread: Option<ThreadSnapshot>,
    pub messages: Vec<ThreadMessageRecord>,
    pub responses: Vec<serde_json::Value>,
}

/// Transport-neutral thread status used by command acknowledgements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentCommandStatus {
    Pending,
    Interrupting,
    Shutdown,
}
