//! Application service wrapping [`AgentControl`].
//!
//! Provides a stable, clone-friendly handle that the API handlers can extract
//! from [`AppState`][crate::context::AppState] via Axum's `State` extractor.

use std::sync::Arc;

use slab_agent::config::AgentConfig;
use slab_agent::control::AgentControl;
use slab_agent::error::AgentError;
use slab_types::ConversationMessage;

use crate::error::ServerError;

/// Thin wrapper around [`AgentControl`] that exposes an application-layer API.
#[derive(Clone)]
pub struct AgentService {
    control: Arc<AgentControl>,
}

impl AgentService {
    pub fn new(control: Arc<AgentControl>) -> Self {
        Self { control }
    }

    /// Spawn a root agent thread.  Returns the new thread ID.
    pub async fn spawn(
        &self,
        session_id: String,
        config: AgentConfig,
        messages: Vec<ConversationMessage>,
    ) -> Result<String, ServerError> {
        self.control
            .spawn(session_id, config, messages)
            .await
            .map_err(agent_err_to_server)
    }

    /// Get the current status snapshot of an agent thread.
    pub async fn get_status(
        &self,
        thread_id: &str,
    ) -> Result<slab_types::agent::AgentThreadStatus, ServerError> {
        let rx = self.control.subscribe(thread_id).await.map_err(agent_err_to_server)?;
        let status = *rx.borrow();
        Ok(status)
    }

    /// Gracefully shut down a running agent thread.
    pub async fn shutdown(&self, thread_id: &str) -> Result<(), ServerError> {
        self.control.shutdown(thread_id).await.map_err(agent_err_to_server)
    }

    /// Return the number of currently active threads.
    pub async fn active_thread_count(&self) -> usize {
        self.control.active_thread_count().await
    }
}

fn agent_err_to_server(e: AgentError) -> ServerError {
    match e {
        AgentError::ThreadNotFound(id) => {
            ServerError::NotFound(format!("agent thread not found: {id}"))
        }
        AgentError::ThreadLimitExceeded { current, max } => {
            ServerError::BadRequest(format!(
                "thread limit exceeded: {current}/{max} concurrent threads active"
            ))
        }
        AgentError::DepthLimitExceeded { current, max } => {
            ServerError::BadRequest(format!("depth limit exceeded: {current}/{max}"))
        }
        other => ServerError::Internal(other.to_string()),
    }
}
