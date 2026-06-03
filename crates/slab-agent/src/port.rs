//! External capability port traits (dependency inversion).
//!
//! The agent orchestration layer never touches SQL, HTTP, or gRPC directly.
//! Instead, the host (`slab-server`) provides concrete adapters that implement
//! these traits and are injected at construction time.

use async_trait::async_trait;

use slab_types::ConversationMessage;
use slab_types::agent::ToolCallStatus;

use crate::config::AgentConfig;
use crate::error::AgentError;
use crate::event::{AgentEventKind, ToolRiskAssessment};

/// Thread lifecycle status, re-exported from `slab_types` for convenience.
pub type ThreadStatus = slab_types::agent::AgentThreadStatus;

// ── Supporting data types ────────────────────────────────────────────────────

/// The response returned by the LLM for a single chat completion call.
#[derive(Debug, Clone)]
pub struct LlmResponse {
    /// Optional assistant text content.
    pub content: Option<String>,
    /// True when `content` was already emitted via [`LlmStreamObserver::on_text_delta`].
    pub content_already_streamed: bool,
    /// Tool calls requested by the model, if any.
    pub tool_calls: Vec<ParsedToolCall>,
    /// The finish reason reported by the provider (e.g. "stop", "tool_calls").
    pub finish_reason: Option<String>,
}

/// A single tool call parsed from the LLM response.
#[derive(Debug, Clone)]
pub struct ParsedToolCall {
    /// Provider-assigned call identifier.
    pub id: String,
    /// Name of the tool to invoke.
    pub name: String,
    /// JSON-encoded arguments string.
    pub arguments: String,
}

/// Tool description passed to the LLM so it knows what tools are available.
#[derive(Debug, Clone)]
pub struct ToolSpec {
    /// Canonical tool name.
    pub name: String,
    /// Human-readable description shown to the model.
    pub description: String,
    /// JSON Schema for the tool's parameter object.
    pub parameters_schema: serde_json::Value,
}

/// Snapshot of an agent thread suitable for persistence.
#[derive(Debug, Clone)]
pub struct ThreadSnapshot {
    pub id: String,
    pub session_id: String,
    pub parent_id: Option<String>,
    pub depth: u32,
    pub status: ThreadStatus,
    pub role_name: Option<String>,
    /// JSON-serialised [`AgentConfig`].
    pub config_json: String,
    /// Final assistant text, populated on successful completion.
    pub completion_text: Option<String>,
    /// RFC 3339 creation timestamp.
    pub created_at: String,
    /// RFC 3339 last-updated timestamp.
    pub updated_at: String,
}

/// Audit record for a single tool call within an agent thread.
#[derive(Debug, Clone)]
pub struct ToolCallRecord {
    pub id: String,
    pub thread_id: String,
    pub tool_name: String,
    /// JSON-encoded arguments string.
    pub arguments: String,
    pub output: Option<String>,
    pub status: ToolCallStatus,
    /// RFC 3339 creation timestamp.
    pub created_at: String,
    /// RFC 3339 completion timestamp, if finished.
    pub completed_at: Option<String>,
}

/// Persisted conversation message for an agent thread.
#[derive(Debug, Clone)]
pub struct ThreadMessageRecord {
    pub id: String,
    pub thread_id: String,
    pub turn_index: u32,
    pub message: ConversationMessage,
    /// RFC 3339 creation timestamp.
    pub created_at: String,
}

/// A streaming event emitted during a single LLM turn.
#[derive(Debug, Clone)]
pub enum TurnEvent {
    Response { turn_index: Option<u32>, event: AgentEventKind },
}

// ── Approval ──────────────────────────────────────────────────────────────────

/// Decision returned by an [`ApprovalPort`] implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approved,
    Rejected,
}

/// Port that lets the host review and approve sensitive tool calls before they
/// are executed.
///
/// Typically implemented by the SSE notification adapter so that an external
/// operator can inspect the command and send an approval via the HTTP API.
#[async_trait]
pub trait ApprovalPort: Send + Sync {
    /// Request approval for a pending tool call.
    ///
    /// The call blocks until the host sends a decision (or the implementation
    /// chooses to auto-approve / auto-reject after a timeout).
    async fn request_approval(
        &self,
        thread_id: &str,
        call_id: &str,
        tool_name: &str,
        command: &str,
        risk: Option<ToolRiskAssessment>,
    ) -> ApprovalDecision;
}

// ── Port traits ──────────────────────────────────────────────────────────────

/// Port for calling chat completions.
///
/// The host provides an adapter that wraps its `ChatService` / `GrpcGateway`.
#[async_trait]
pub trait LlmPort: Send + Sync {
    /// Perform a single chat completion round-trip and return the response.
    async fn chat_completion(
        &self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        config: &AgentConfig,
    ) -> Result<LlmResponse, AgentError>;

    /// Perform a chat completion while forwarding visible text deltas as they
    /// become available.
    ///
    /// Implementations that cannot stream should keep the default behavior; it
    /// emits one final delta after the blocking completion returns.
    async fn chat_completion_streaming(
        &self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        config: &AgentConfig,
        observer: &mut dyn LlmStreamObserver,
    ) -> Result<LlmResponse, AgentError> {
        let mut response = self.chat_completion(model, messages, tools, config).await?;
        if response.tool_calls.is_empty()
            && let Some(content) = response.content.as_deref()
            && !content.is_empty()
        {
            observer.on_text_delta(content).await?;
            response.content_already_streamed = true;
        }
        Ok(response)
    }
}

/// Receives model deltas from an [`LlmPort`] streaming implementation.
#[async_trait]
pub trait LlmStreamObserver: Send {
    /// Called with assistant text that is safe to show to the caller.
    async fn on_text_delta(&mut self, delta: &str) -> Result<(), AgentError>;

    /// Called with assistant reasoning text as it becomes available.
    async fn on_reasoning_delta(&mut self, _delta: &str) -> Result<(), AgentError> {
        Ok(())
    }

    /// Called once with the final assistant reasoning text.
    async fn on_reasoning_done(&mut self, _text: &str) -> Result<(), AgentError> {
        Ok(())
    }
}

/// Port for persisting agent state.
///
/// The host provides an adapter that wraps its SQLx-backed store.
#[async_trait]
pub trait AgentStorePort: Send + Sync {
    /// Insert or update a thread snapshot.
    async fn upsert_thread(&self, snapshot: &ThreadSnapshot) -> Result<(), AgentError>;

    /// Retrieve a thread snapshot by ID.
    async fn get_thread(&self, id: &str) -> Result<Option<ThreadSnapshot>, AgentError>;

    /// Return root thread snapshots for a chat session, newest first.
    async fn list_session_threads(
        &self,
        session_id: &str,
    ) -> Result<Vec<ThreadSnapshot>, AgentError>;

    /// Update only the status (and optional completion text) of an existing thread.
    async fn update_thread_status(
        &self,
        id: &str,
        status: ThreadStatus,
        completion_text: Option<&str>,
    ) -> Result<(), AgentError>;

    /// Insert a new tool call audit record.
    async fn insert_tool_call(&self, record: &ToolCallRecord) -> Result<(), AgentError>;

    /// Update an existing tool call record with its output and final status.
    async fn update_tool_call(
        &self,
        id: &str,
        output: Option<&str>,
        status: ToolCallStatus,
        completed_at: &str,
    ) -> Result<(), AgentError>;

    /// Insert a conversation message for a thread.
    async fn insert_thread_message(&self, record: &ThreadMessageRecord) -> Result<(), AgentError>;

    /// Return persisted conversation messages for a thread in replay order.
    async fn list_thread_messages(
        &self,
        thread_id: &str,
    ) -> Result<Vec<ThreadMessageRecord>, AgentError>;
}

/// Port for status-change and turn-event notifications.
///
/// The host provides an adapter that fans out to SSE streams, WebSockets, etc.
#[async_trait]
pub trait AgentNotifyPort: Send + Sync {
    /// Called whenever a thread transitions to a new [`ThreadStatus`].
    async fn on_status_change(&self, thread_id: &str, status: ThreadStatus);

    /// Called for each [`TurnEvent`] emitted during an LLM turn.
    ///
    /// The default implementation is a no-op so existing adapters that only
    /// care about status changes do not need to be updated.
    async fn on_turn_event(&self, _thread_id: &str, _event: &TurnEvent) {}
}
