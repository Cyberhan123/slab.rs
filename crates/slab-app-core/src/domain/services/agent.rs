//! Application service wrapping [`AgentControl`].
//!
//! Provides a stable, clone-friendly handle that the API handlers can extract
//! from [`AppState`][crate::context::AppState] via Axum's `State` extractor.

use std::sync::Arc;

use slab_agent::config::AgentConfig;
use slab_agent::control::AgentControl;
use slab_agent::error::AgentError;
use slab_agent::port::{AgentStorePort, ThreadMessageRecord, ThreadSnapshot};
use slab_types::ConversationMessage;

use crate::error::AppCoreError;
use crate::infra::agent::event_hub::{AgentEventHub, AgentEventSubscription};

/// Thin wrapper around [`AgentControl`] that exposes an application-layer API.
#[derive(Clone)]
pub struct AgentService {
    control: Arc<AgentControl>,
    store: Arc<dyn AgentStorePort>,
    events: Arc<AgentEventHub>,
}

/// Persisted session state restored by the unified agent responses route.
pub struct RestoredAgentSession {
    pub thread: Option<ThreadSnapshot>,
    pub messages: Vec<ThreadMessageRecord>,
}

impl AgentService {
    pub fn new(
        control: Arc<AgentControl>,
        store: Arc<dyn AgentStorePort>,
        events: Arc<AgentEventHub>,
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
        self.control.spawn(session_id, config, messages).await.map_err(AppCoreError::from)
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
            Err(e) => return Err(AppCoreError::from(e)),
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
        self.control.shutdown(thread_id).await.map_err(AppCoreError::from)
    }

    /// Interrupt the currently running turn while keeping the thread resumable.
    pub async fn interrupt(&self, thread_id: &str) -> Result<(), AppCoreError> {
        self.control.interrupt(thread_id).await.map_err(AppCoreError::from)
    }

    /// Append user input to an existing agent thread and run the next turn.
    pub async fn send_input(&self, thread_id: &str, content: String) -> Result<(), AppCoreError> {
        self.control.send_input(thread_id, content).await.map_err(AppCoreError::from)
    }

    /// List persisted root agent threads for a chat session, newest first.
    pub async fn list_session_threads(
        &self,
        session_id: &str,
    ) -> Result<Vec<ThreadSnapshot>, AppCoreError> {
        self.store
            .list_session_threads(session_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))
    }

    /// Restore the latest root thread for a chat session and its persisted messages.
    pub async fn restore_session(
        &self,
        session_id: &str,
    ) -> Result<RestoredAgentSession, AppCoreError> {
        let thread = self.list_session_threads(session_id).await?.into_iter().next();
        let messages = match thread.as_ref() {
            Some(thread) => self.list_thread_messages(&thread.id).await?,
            None => Vec::new(),
        };
        Ok(RestoredAgentSession { thread, messages })
    }

    /// List persisted messages for a thread in replay order.
    pub async fn list_thread_messages(
        &self,
        thread_id: &str,
    ) -> Result<Vec<ThreadMessageRecord>, AppCoreError> {
        if self
            .store
            .get_thread(thread_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?
            .is_none()
        {
            return Err(AppCoreError::NotFound(format!("agent thread not found: {thread_id}")));
        }

        self.store
            .list_thread_messages(thread_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))
    }

    /// Subscribe to the turn-event stream for a thread.
    ///
    /// Returns a broadcast receiver that replays events emitted after the call.
    pub fn subscribe_events(&self, thread_id: &str) -> AgentEventSubscription {
        self.events.subscribe_events(thread_id)
    }

    /// Send an approval decision for a pending tool-call.
    ///
    /// Both `thread_id` (from the URL path) and `call_id` must match so that
    /// approvals cannot be delivered to a different thread's pending call.
    ///
    /// Returns `true` if a pending approval with the given key was found and
    /// the decision was delivered.
    pub fn approve_call(&self, thread_id: &str, call_id: &str, approved: bool) -> bool {
        self.events.approve_call(thread_id, call_id, approved)
    }

    /// Return the number of currently active threads.
    #[allow(dead_code)]
    pub async fn active_thread_count(&self) -> usize {
        self.control.active_thread_count().await
    }

    pub(crate) fn control(&self) -> Arc<AgentControl> {
        Arc::clone(&self.control)
    }
}
