//! Single agent thread lifecycle.

use std::sync::Arc;

use chrono::Utc;
use tokio::sync::watch;
use tracing::{debug, error, info};
use uuid::Uuid;

use slab_types::{ConversationMessage, ConversationMessageContent};

use crate::{
    config::AgentConfig,
    error::AgentError,
    port::{AgentNotifyPort, AgentStorePort, LlmPort, ThreadSnapshot, ThreadStatus},
    tool::ToolRouter,
    turn::{TurnExecutionContext, execute_turn},
};

/// A single agent conversation thread.
///
/// Created by [`crate::control::AgentControl`] and consumed by [`AgentThread::run`].
pub struct AgentThread {
    /// Unique thread identifier.
    pub id: String,
    /// The chat session this thread belongs to.
    pub session_id: String,
    /// Parent thread ID for sub-agents; `None` for root agents.
    pub parent_id: Option<String>,
    /// Nesting depth (0 = root).
    pub depth: u32,
    /// Runtime configuration for this thread.
    pub config: AgentConfig,
    /// Shared sender so the controller can also signal status changes (e.g. Shutdown).
    pub(crate) status_tx: Arc<watch::Sender<ThreadStatus>>,
}

impl AgentThread {
    /// Create a new thread and return it together with a status [`watch::Receiver`].
    pub fn new(
        session_id: String,
        parent_id: Option<String>,
        depth: u32,
        config: AgentConfig,
    ) -> (Self, watch::Receiver<ThreadStatus>) {
        let id = Uuid::new_v4().to_string();
        let (status_tx_inner, status_rx) = watch::channel(ThreadStatus::Pending);
        let status_tx = Arc::new(status_tx_inner);
        let thread = Self { id, session_id, parent_id, depth, config, status_tx };
        (thread, status_rx)
    }

    /// Subscribe to status changes for this thread.
    pub fn subscribe(&self) -> watch::Receiver<ThreadStatus> {
        self.status_tx.subscribe()
    }

    /// Run the agent loop to completion, consuming `self`.
    ///
    /// Injects the system prompt (if configured), then loops over LLM turns
    /// until the model produces a final answer, `max_turns` is exhausted, or
    /// an error occurs.
    ///
    /// Returns the final assistant text on success.
    pub async fn run(
        self,
        mut messages: Vec<ConversationMessage>,
        llm: Arc<dyn LlmPort>,
        store: Arc<dyn AgentStorePort>,
        notify: Arc<dyn AgentNotifyPort>,
        tools: Arc<ToolRouter>,
    ) -> Result<String, AgentError> {
        let thread_id = self.id.clone();
        let now = Utc::now().to_rfc3339();

        // Fail early if the config cannot be serialized — a swallowed error here
        // would silently persist an empty config_json and make debugging impossible.
        let config_json = serde_json::to_string(&self.config)
            .map_err(|e| AgentError::Internal(format!("failed to serialize agent config: {e}")))?;

        // Persist initial snapshot.
        let snapshot = ThreadSnapshot {
            id: thread_id.clone(),
            session_id: self.session_id.clone(),
            parent_id: self.parent_id.clone(),
            depth: self.depth,
            status: ThreadStatus::Pending,
            role_name: None,
            config_json,
            completion_text: None,
            created_at: now.clone(),
            updated_at: now,
        };
        if let Err(e) = store.upsert_thread(&snapshot).await {
            error!(thread_id, error = %e, "failed to persist thread snapshot");
        }

        self.set_status(ThreadStatus::Running, &notify).await;

        // Persist the Running transition so the stored status matches the in-memory state.
        if let Err(e) = store.update_thread_status(&thread_id, ThreadStatus::Running, None).await {
            error!(thread_id, error = %e, "failed to persist running status");
        }

        // Inject system prompt as the first message, if not already present.
        if let Some(ref system_prompt) = self.config.system_prompt {
            if !system_prompt.is_empty()
                && messages.first().map(|m| m.role.as_str()) != Some("system")
            {
                messages.insert(
                    0,
                    ConversationMessage {
                        role: "system".to_owned(),
                        content: ConversationMessageContent::Text(system_prompt.clone()),
                        name: None,
                        tool_call_id: None,
                        tool_calls: vec![],
                    },
                );
            }
        }

        let mut completion_text: Option<String> = None;
        let mut last_error: Option<AgentError> = None;

        'turns: for turn_index in 0..self.config.max_turns {
            debug!(thread_id, turn_index, "starting turn");
            match execute_turn(
                TurnExecutionContext {
                    thread_id: &thread_id,
                    turn_index,
                    depth: self.depth,
                    config: &self.config,
                    llm: llm.as_ref(),
                    tools: tools.as_ref(),
                    store: store.as_ref(),
                },
                &mut messages,
            )
            .await
            {
                Ok(more_turns) => {
                    if !more_turns {
                        // Extract the final assistant text.
                        completion_text = messages.iter().rev().find_map(|m| {
                            if m.role == "assistant" {
                                if let ConversationMessageContent::Text(ref t) = m.content {
                                    if !t.is_empty() {
                                        return Some(t.clone());
                                    }
                                }
                            }
                            None
                        });
                        break 'turns;
                    }
                }
                Err(e) => {
                    error!(thread_id, turn_index, error = %e, "turn failed");
                    last_error = Some(e);
                    break 'turns;
                }
            }
        }

        if let Some(err) = last_error {
            self.set_status(ThreadStatus::Errored, &notify).await;
            store
                .update_thread_status(&thread_id, ThreadStatus::Errored, Some(&err.to_string()))
                .await
                .ok();
            return Err(err);
        }

        info!(thread_id, "thread completed");
        self.set_status(ThreadStatus::Completed, &notify).await;
        store
            .update_thread_status(&thread_id, ThreadStatus::Completed, completion_text.as_deref())
            .await
            .ok();

        Ok(completion_text.unwrap_or_default())
    }

    async fn set_status(&self, status: ThreadStatus, notify: &Arc<dyn AgentNotifyPort>) {
        let _ = self.status_tx.send(status);
        notify.on_status_change(&self.id, status).await;
    }
}
