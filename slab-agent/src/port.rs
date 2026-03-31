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

/// Thread lifecycle status, re-exported from `slab_types` for convenience.
pub type ThreadStatus = slab_types::agent::AgentThreadStatus;

// ── Supporting data types ────────────────────────────────────────────────────

/// The response returned by the LLM for a single chat completion call.
#[derive(Debug, Clone)]
pub struct LlmResponse {
    /// Optional assistant text content.
    pub content: Option<String>,
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
}

/// Port for status-change notifications.
///
/// The host provides an adapter that fans out to SSE streams, WebSockets, etc.
#[async_trait]
pub trait AgentNotifyPort: Send + Sync {
    /// Called whenever a thread transitions to a new [`ThreadStatus`].
    async fn on_status_change(&self, thread_id: &str, status: ThreadStatus);
}
