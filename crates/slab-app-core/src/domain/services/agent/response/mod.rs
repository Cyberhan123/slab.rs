//! Response service — OpenAI Responses wire host for `/responses`.
//!
//! Produces the canonical OpenAI Responses wire directly (no separate
//! projection layer): the non-streaming [`build_response`] assembler lives in
//! [`projection`], the streaming [`envelope_to_events`] state machine in
//! [`stream`]. Both are re-exported here so callers (the server transport,
//! tests) can reach them through `domain::services::agent::response`.
//!
//! Holds a cheap clone of the shared [`AgentCore`]; today the `/responses` run
//! still flows through the slab-agent turn loop (`spawn` / `send_input`). A
//! later slice evolves this into a standalone single-shot Model.

pub mod projection;
pub mod stream;

pub use projection::{
    AdapterInput, build_response, parse_mcp_status, parse_phase, parse_shell_output_content,
};
pub use stream::{StreamCtx, envelope_to_events};

use super::AgentCore;
use crate::domain::models::{AgentCommand, AgentSessionSnapshot};
use crate::error::AppCoreError;
use crate::infra::agent::event_hub::AgentEventSubscription;

/// Response-side agent service: owns the OpenAI Responses wire surface
/// consumed by the `/responses` HTTP / SSE / WebSocket transport.
#[derive(Clone)]
pub struct ResponseService(AgentCore);

impl ResponseService {
    pub(crate) fn new(core: AgentCore) -> Self {
        Self(core)
    }

    /// Handle one transport-neutral agent command.
    ///
    /// HTTP, WebSocket, and other callers should enter the agent use case here
    /// after converting their wire DTOs into [`AgentCommand`].
    pub async fn handle_command(&self, command: AgentCommand) -> Result<String, AppCoreError> {
        match command {
            AgentCommand::CreateResponse { session_id, config, messages } => {
                self.0.spawn(session_id, *config, messages).await
            }
            AgentCommand::AppendInput { thread_id, content } => {
                self.0.send_input(&thread_id, content).await?;
                Ok(thread_id)
            }
        }
    }

    pub async fn restore_session_snapshot(
        &self,
        session_id: &str,
    ) -> Result<AgentSessionSnapshot, AppCoreError> {
        let restored = self.0.restore_session(session_id).await?;
        Ok(AgentSessionSnapshot {
            session_id: session_id.to_owned(),
            thread: restored.thread,
            messages: restored.messages,
            responses: restored.responses,
        })
    }

    /// Subscribe to the turn-event stream for a thread.
    pub fn subscribe_events(&self, thread_id: &str) -> AgentEventSubscription {
        self.0.subscribe_events(thread_id)
    }
}
