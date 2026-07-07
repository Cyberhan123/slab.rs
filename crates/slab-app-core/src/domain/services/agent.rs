//! Application service wrapping [`slab_agent::AgentRuntime`].
//!
//! Provides a stable, clone-friendly handle that the API handlers can extract
//! from [`AppState`][crate::context::AppState] via Axum's `State` extractor.

use std::path::Path;
use std::sync::Arc;

use slab_agent::AgentRuntime;
use slab_agent::config::AgentConfig;
use slab_agent::error::AgentError;
use slab_agent::port::{AgentStorePort, ThreadMessageRecord, ThreadSnapshot};
use slab_types::ConversationMessage;
use slab_utils::session_snapshot::{
    build_migration_snapshot, project_id_from_root, write_session_snapshot_atomic,
};

use crate::domain::models::{
    AgentCommand, AgentControlCommand, AgentControlResult, AgentControlStatus, AgentSessionSnapshot,
};
use crate::error::AppCoreError;
use crate::infra::agent::event_hub::{AgentEventHub, AgentEventSubscription};
use crate::infra::db::repository::agent_response::AgentResponseStore;

/// Thin wrapper around [`AgentRuntime`] that exposes an application-layer API.
#[derive(Clone)]
pub struct AgentService {
    runtime: AgentRuntime,
    store: Arc<dyn AgentStorePort>,
    response_store: Arc<dyn AgentResponseStore>,
    events: Arc<AgentEventHub>,
}

/// Persisted session state restored by the unified agent responses route.
pub struct RestoredAgentSession {
    pub thread: Option<ThreadSnapshot>,
    pub messages: Vec<ThreadMessageRecord>,
    /// Complete OpenAI-Responses-canonical `Response` JSON objects, one per
    /// agent run, oldest first. Empty for pre-migration history.
    pub responses: Vec<serde_json::Value>,
}

impl AgentService {
    pub fn new(
        runtime: AgentRuntime,
        store: Arc<dyn AgentStorePort>,
        response_store: Arc<dyn AgentResponseStore>,
        events: Arc<AgentEventHub>,
    ) -> Self {
        Self { runtime, store, response_store, events }
    }

    /// Spawn a root agent thread.  Returns the new thread ID.
    pub async fn spawn(
        &self,
        session_id: String,
        config: AgentConfig,
        messages: Vec<ConversationMessage>,
    ) -> Result<String, AppCoreError> {
        self.runtime.create_response(session_id, config, messages).await.map_err(AppCoreError::from)
    }

    /// Handle one transport-neutral agent command.
    ///
    /// HTTP, WebSocket, and other callers should enter the agent use case here
    /// after converting their wire DTOs into [`AgentCommand`].
    pub async fn handle_command(&self, command: AgentCommand) -> Result<String, AppCoreError> {
        match command {
            AgentCommand::CreateResponse { session_id, config, messages } => {
                self.spawn(session_id, config, messages).await
            }
            AgentCommand::AppendInput { thread_id, content } => {
                self.send_input(&thread_id, content).await?;
                Ok(thread_id)
            }
        }
    }

    pub async fn restore_session_snapshot(
        &self,
        session_id: &str,
    ) -> Result<AgentSessionSnapshot, AppCoreError> {
        let restored = self.restore_session(session_id).await?;
        Ok(AgentSessionSnapshot {
            session_id: session_id.to_owned(),
            thread: restored.thread,
            messages: restored.messages,
            responses: restored.responses,
        })
    }

    pub async fn handle_control(
        &self,
        command: AgentControlCommand,
    ) -> Result<AgentControlResult, AppCoreError> {
        match command {
            AgentControlCommand::ResolveApproval { thread_id, call_id, approved } => {
                let delivered = self.approve_call(&thread_id, &call_id, approved);
                Ok(AgentControlResult { thread_id, delivered: Some(delivered), status: None })
            }
            AgentControlCommand::Interrupt { thread_id } => {
                self.interrupt(&thread_id).await?;
                Ok(AgentControlResult {
                    thread_id,
                    delivered: None,
                    status: Some(AgentControlStatus::Interrupting),
                })
            }
            AgentControlCommand::Shutdown { thread_id } => {
                self.shutdown(&thread_id).await?;
                Ok(AgentControlResult {
                    thread_id,
                    delivered: None,
                    status: Some(AgentControlStatus::Shutdown),
                })
            }
        }
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
        match self.runtime.subscribe(thread_id).await {
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
        self.runtime.shutdown(thread_id).await.map_err(AppCoreError::from)
    }

    /// Interrupt the currently running turn while keeping the thread resumable.
    pub async fn interrupt(&self, thread_id: &str) -> Result<(), AppCoreError> {
        self.runtime.interrupt(thread_id).await.map_err(AppCoreError::from)
    }

    /// Append user input to an existing agent thread and run the next turn.
    pub async fn send_input(&self, thread_id: &str, content: String) -> Result<(), AppCoreError> {
        self.runtime.append_input(thread_id, content).await.map_err(AppCoreError::from)
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
        let (messages, responses) = match thread.as_ref() {
            Some(thread) => {
                let messages = self.list_thread_messages(&thread.id).await?;
                let responses = self
                    .response_store
                    .list_thread_responses(&thread.id)
                    .await
                    .map_err(|e| AppCoreError::Internal(e.to_string()))?
                    .into_iter()
                    .filter_map(|record| {
                        serde_json::from_str::<serde_json::Value>(&record.response_json)
                            .map_err(|error| {
                                tracing::warn!(
                                    run_id = %record.run_id,
                                    thread_id = %record.thread_id,
                                    %error,
                                    "stored agent response_json is not valid JSON; skipping"
                                );
                                error
                            })
                            .ok()
                    })
                    .collect::<Vec<_>>();
                (messages, responses)
            }
            None => (Vec::new(), Vec::new()),
        };
        Ok(RestoredAgentSession { thread, messages, responses })
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
        self.runtime.active_thread_count().await
    }

    /// Prepare a workspace switch (B-8 / INFRA-01): interrupt every active agent
    /// thread, then write a project-scoped atomic snapshot of the interrupted
    /// threads so a future restore only resumes threads that belong to the
    /// originating workspace. Returns the project id + the number of threads that
    /// were suspended. Any failure aborts the migration (the caller must not
    /// proceed to switch workspaces on error).
    pub async fn prepare_workspace_migration(
        &self,
        workspace_root: &Path,
        snapshot_dir: &Path,
    ) -> Result<WorkspaceMigrationOutcome, AppCoreError> {
        let suspended = self.runtime.interrupt_all().await;
        let project_id = project_id_from_root(workspace_root);
        let snapshot = build_migration_snapshot(&project_id, &suspended);
        write_session_snapshot_atomic(snapshot_dir, &snapshot).map_err(AppCoreError::Internal)?;
        Ok(WorkspaceMigrationOutcome { project_id, suspended_count: suspended.len() })
    }

    pub(crate) fn runtime(&self) -> AgentRuntime {
        self.runtime.clone()
    }
}

/// Outcome of a workspace migration preparation (B-8).
#[derive(Debug, Clone)]
pub struct WorkspaceMigrationOutcome {
    pub project_id: String,
    pub suspended_count: usize,
}
