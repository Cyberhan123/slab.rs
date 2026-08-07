//! External capability port traits (dependency inversion).
//!
//! The agent orchestration layer never touches SQL, HTTP, or gRPC directly.
//! Instead, the host (`slab-server`) provides concrete adapters that implement
//! these traits and are injected at construction time.

use serde::{Deserialize, Serialize};

use async_trait::async_trait;

use slab_agent_tracing::AgentTraceContext;
use slab_types::ConversationMessage;

use crate::config::AgentConfig;
use crate::error::AgentError;
use crate::plan::Plan;
use crate::protocol::EventMsg;

/// Thread lifecycle status, re-exported from `slab_types` for convenience.
pub type ThreadStatus = slab_types::agent::AgentThreadStatus;

// ── Supporting data types ────────────────────────────────────────────────────

/// The response returned by the LLM for a single chat completion call.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LlmResponse {
    /// Optional assistant text content.
    pub content: Option<String>,
    /// True when `content` was already emitted via [`LlmStreamObserver::on_text_delta`].
    pub content_already_streamed: bool,
    /// Tool calls requested by the model, if any.
    pub tool_calls: Vec<ParsedToolCall>,
    /// The finish reason reported by the provider (e.g. "stop", "tool_calls").
    pub finish_reason: Option<String>,
    /// Token usage reported by the provider/runtime for this LLM call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<LlmUsage>,
}

/// Token usage reported for a single LLM call.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub estimated: bool,
}

/// A single tool call parsed from the LLM response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParsedToolCall {
    /// Provider-assigned call identifier.
    pub id: String,
    /// Name of the tool to invoke.
    pub name: String,
    /// JSON-encoded arguments string.
    pub arguments: String,
}

/// Tool description passed to the LLM so it knows what tools are available.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    /// RFC 3339 timestamp the thread was archived, or `None` for a live thread.
    /// Archived threads are excluded from `list_session_threads_filtered`
    /// unless the filter opts in via `include_archived`.
    pub archived_at: Option<String>,
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

/// Persisted state snapshot for a single agent turn.
#[derive(Debug, Clone)]
pub struct TurnStateRecord {
    pub thread_id: String,
    pub turn_index: u32,
    pub status: String,
    pub input_messages_json: Option<String>,
    pub tool_specs_json: Option<String>,
    pub llm_response_json: Option<String>,
    pub error: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

/// Persisted full-fidelity harness `TurnItem` snapshot (one finalized item).
///
/// `item_json` is the serialized `slab_proto::harness::item::TurnItem`. The
/// store layer treats it as opaque JSON so `slab-agent` stays free of the
/// harness proto; decoding back to `TurnItem` happens in the application layer.
/// `seq` orders items within `(thread_id, turn_index)` for deterministic replay.
#[derive(Debug, Clone)]
pub struct TurnItemRecord {
    pub id: String,
    pub thread_id: String,
    pub turn_index: u32,
    pub seq: u32,
    pub item_json: String,
    /// RFC 3339 creation timestamp.
    pub created_at: String,
}

/// Risk level assigned to a tool call by the risk analyzer.
///
/// Relocated from `event.rs` — `event.rs` was deleted when the
/// OpenAI-Responses wire vocabulary (`AgentEventKind`) left the crate. Risk
/// assessment stays in slab-agent because it is part of the `ApprovalPort`
/// surface and the `risk` analyzer, not the response wire.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ToolRiskLevel {
    Low,
    Medium,
    High,
}

/// Structured risk assessment for a tool call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolRiskAssessment {
    pub level: ToolRiskLevel,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

// ── Approval ──────────────────────────────────────────────────────────────────

/// Decision returned by an [`ApprovalPort`] implementation. An approval carries
/// the user-chosen [`slab_exec_policy::ApprovalScope`] so the kernel can persist the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approved(slab_exec_policy::ApprovalScope),
    Rejected,
}

/// Re-export the unified exec-policy port so the kernel can hold
/// `Arc<dyn ExecPolicyPort>` without a separate import.
pub use slab_exec_policy::ExecPolicyPort;

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
    /// chooses to auto-approve / auto-reject after a timeout). The returned
    /// [`ApprovalDecision`] carries the user's persistence scope.
    async fn request_approval(
        &self,
        thread_id: &str,
        call_id: &str,
        tool_name: &str,
        descriptor: &slab_exec_policy::OperationDescriptor,
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
        trace_context: &AgentTraceContext,
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
        trace_context: &AgentTraceContext,
        observer: &mut dyn LlmStreamObserver,
    ) -> Result<LlmResponse, AgentError> {
        let mut response =
            self.chat_completion(model, messages, tools, config, trace_context).await?;
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

/// Filter / pagination parameters for [`AgentStorePort::list_session_threads_filtered`].
#[derive(Debug, Clone, Default)]
pub struct ThreadListFilter {
    /// Maximum number of threads to return.
    pub limit: Option<u32>,
    /// Cursor: only return threads whose `updated_at` sorts strictly before this
    /// RFC 3339 timestamp (enables `next_cursor`-style pagination).
    pub before_updated_at: Option<String>,
    /// Include archived threads in the result (default excludes them).
    pub include_archived: bool,
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

    /// Return root thread snapshots for a session, honoring a [`ThreadListFilter`]
    /// (limit + cursor pagination, archived inclusion). Hosts that do not support
    /// filtering fall back to the unfiltered [`Self::list_session_threads`].
    async fn list_session_threads_filtered(
        &self,
        session_id: &str,
        filter: &ThreadListFilter,
    ) -> Result<Vec<ThreadSnapshot>, AgentError> {
        // Default ignores the filter so existing adapters keep compiling.
        let _ = filter;
        self.list_session_threads(session_id).await
    }

    /// Update only the status (and optional completion text) of an existing thread.
    async fn update_thread_status(
        &self,
        id: &str,
        status: ThreadStatus,
        completion_text: Option<&str>,
    ) -> Result<(), AgentError>;

    /// Mark a thread archived (`Some`) or restore it (`None`).
    ///
    /// Hosts that do not support archiving can keep this default no-op.
    async fn archive_thread(
        &self,
        _id: &str,
        _archived_at: Option<&str>,
    ) -> Result<(), AgentError> {
        Ok(())
    }
}

/// Host-inferred memory-pressure state for spawn admission (INFRA-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure {
    /// Memory is within budget; spawns may proceed.
    Nominal,
    /// Memory exceeded the configured threshold; new spawns should pause until
    /// the breaker clears after a cooldown.
    Tripped { current_mb: u64, threshold_mb: u64 },
}

/// Port that reports memory pressure to gate agent spawns (INFRA-05).
///
/// Keeps `slab-agent` free of `sysinfo`/process concerns: the host (app-core)
/// owns the circuit breaker that samples process RSS and exposes its state here.
/// The default [`NoopMemoryPressurePort`] never trips, preserving the legacy
/// admission behavior when no breaker is wired.
pub trait MemoryPressurePort: Send + Sync {
    /// Return the current memory-pressure state.
    fn check(&self) -> MemoryPressure;
}

/// [`MemoryPressurePort`] that never trips (no breaker wired).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopMemoryPressurePort;

impl MemoryPressurePort for NoopMemoryPressurePort {
    fn check(&self) -> MemoryPressure {
        MemoryPressure::Nominal
    }
}

/// Port for dispatching plugin agent-capability calls (B-7 / ADR-009).
///
/// Keeps `slab-agent` free of plugin/runtime concerns: the host composition
/// root (app-core) provides an adapter that routes a `plugin__<id>__<cap>`
/// tool call to the supervised plugin runtime through this port. Plugins
/// cannot self-report effects/trust — the host derives the isolation tier from
/// the plugin's runtime kind and registers the proxy tool.
#[async_trait]
pub trait PluginToolPort: Send + Sync {
    /// Invoke `capability_id` on `plugin_id` with JSON `arguments`.
    ///
    /// Returns the plugin's JSON-serialised result string. The adapter resolves
    /// the capability's transport function and routes to the correct runtime
    /// (js / python / wasm).
    async fn call_capability(
        &self,
        plugin_id: &str,
        capability_id: &str,
        arguments: &serde_json::Value,
    ) -> Result<String, AgentError>;
}

/// Port for the per-thread plan store backing Plan interaction mode.
///
/// Keeps `slab-agent` free of storage concerns: the host (app-core) provides an
/// in-memory per-thread map. The store is the single source of truth for the
/// durable plan a thread authors with the `plan` / `update_plan` tools and
/// presents via `present_plan`. Keyed by thread id so plans are isolated per
/// thread and cleared on teardown (alongside the exec-policy per-thread state).
#[async_trait]
pub trait PlanStorePort: Send + Sync {
    /// Replace the thread's current plan (creates or overwrites).
    async fn replace_plan(&self, thread_id: &str, plan: Plan) -> Result<(), AgentError>;

    /// Read the thread's current plan, if any.
    async fn current_plan(&self, thread_id: &str) -> Option<Plan>;

    /// Drop the thread's plan (called on thread teardown).
    async fn clear(&self, thread_id: &str);
}

/// [`PlanStorePort`] that stores nothing — the default for [`crate::ToolContext`]
/// in tests and legacy paths so existing tool tests keep compiling without a
/// concrete store wired.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopPlanStore;

#[async_trait]
impl PlanStorePort for NoopPlanStore {
    async fn replace_plan(&self, _thread_id: &str, _plan: Plan) -> Result<(), AgentError> {
        Ok(())
    }
    async fn current_plan(&self, _thread_id: &str) -> Option<Plan> {
        None
    }
    async fn clear(&self, _thread_id: &str) {}
}

/// Port for status-change and harness-protocol notifications.
///
/// The host provides an adapter that fans out to SSE streams, WebSockets, etc.
#[async_trait]
pub trait AgentNotifyPort: Send + Sync {
    /// Called whenever a thread transitions to a new [`ThreadStatus`].
    async fn on_status_change(&self, thread_id: &str, status: ThreadStatus);

    /// Called for each harness-protocol [`EventMsg`] the agent emits directly.
    ///
    /// This is the harness surface (`slab-agent::protocol`): turn lifecycle,
    /// assistant text/reasoning, and tool items. Adapters that fan harness
    /// events out to a client override this; the default is a no-op.
    async fn on_event_msg(&self, _thread_id: &str, _msg: &EventMsg) {}
}
