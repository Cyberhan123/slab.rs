//! Application service wrapping [`AgentControl`].
//!
//! Provides a stable, clone-friendly handle that the API handlers can extract
//! from [`AppState`][crate::context::AppState] via Axum's `State` extractor.

use std::sync::Arc;

use slab_agent::config::AgentConfig;
use slab_agent::control::AgentControl;
use slab_agent::error::AgentError;
use slab_agent::port::{AgentStorePort, TurnEvent};
use slab_types::ConversationMessage;
use tokio::sync::broadcast;

use crate::error::AppCoreError;
use crate::infra::sse_notify::SseNotifyAdapter;

/// Thin wrapper around [`AgentControl`] that exposes an application-layer API.
#[derive(Clone)]
pub struct AgentService {
    control: Arc<AgentControl>,
    store: Arc<dyn AgentStorePort>,
    events: Arc<SseNotifyAdapter>,
}

impl AgentService {
    pub fn new(
        control: Arc<AgentControl>,
        store: Arc<dyn AgentStorePort>,
        events: Arc<SseNotifyAdapter>,
    ) -> Self {
        Self { control, store, events }
    }

    /// Spawn a root agent thread.  Returns the new thread ID.
    pub async fn spawn(
        &self,
        session_id: String,
        config: AgentConfig,
        messages: Vec<ConversationMessage>,
    ) -> Result<String, AppCoreError> {
        self.control.spawn(session_id, config, messages).await.map_err(agent_err_to_server)
    }

    /// Get the current status of an agent thread.
    ///
    /// First checks the in-memory registry (for live threads), then falls back
    /// to the persisted snapshot so callers polling after completion still get
    /// an accurate status rather than a 404.
    pub async fn get_status(
        &self,
        thread_id: &str,
    ) -> Result<slab_types::agent::AgentThreadStatus, AppCoreError> {
        // Try the live in-memory registry first.
        match self.control.subscribe(thread_id).await {
            Ok(rx) => {
                return Ok(*rx.borrow());
            }
            Err(AgentError::ThreadNotFound(_)) => {
                // Thread has already finished and was removed from the registry.
                // Fall through to the DB lookup below.
            }
            Err(e) => return Err(agent_err_to_server(e)),
        }

        // Fallback: look up the persisted snapshot.
        match self.store.get_thread(thread_id).await {
            Ok(Some(snapshot)) => Ok(snapshot.status),
            Ok(None) => Err(AppCoreError::NotFound(format!("agent thread not found: {thread_id}"))),
            Err(e) => Err(AppCoreError::Internal(e.to_string())),
        }
    }

    /// Gracefully shut down a running agent thread.
    pub async fn shutdown(&self, thread_id: &str) -> Result<(), AppCoreError> {
        self.control.shutdown(thread_id).await.map_err(agent_err_to_server)
    }

    /// Subscribe to the turn-event stream for a thread.
    ///
    /// Returns a broadcast receiver that replays events emitted after the call.
    pub fn subscribe_events(&self, thread_id: &str) -> broadcast::Receiver<TurnEvent> {
        self.events.subscribe_events(thread_id)
    }

    /// Send an approval decision for a pending tool-call.
    ///
    /// Returns `true` if a pending approval with the given `call_id` was found
    /// and the decision was delivered.
    pub fn approve_call(&self, call_id: &str, approved: bool) -> bool {
        self.events.approve_call(call_id, approved)
    }

    /// Return the number of currently active threads.
    #[allow(dead_code)]
    pub async fn active_thread_count(&self) -> usize {
        self.control.active_thread_count().await
    }
}

fn agent_err_to_server(e: AgentError) -> AppCoreError {
    match e {
        AgentError::ThreadNotFound(id) => {
            AppCoreError::NotFound(format!("agent thread not found: {id}"))
        }
        AgentError::ThreadLimitExceeded { current, max } => AppCoreError::TooManyRequests(format!(
            "thread limit exceeded: {current}/{max} concurrent threads active"
        )),
        AgentError::DepthLimitExceeded { current, max } => {
            AppCoreError::BadRequest(format!("depth limit exceeded: {current}/{max}"))
        }
        other => AppCoreError::Internal(other.to_string()),
    }
}
