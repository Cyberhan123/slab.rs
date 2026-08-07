//! Harness notification param shapes (the `EventMsg` payloads).
//!
//! These are the authoritative payloads: they are wrapped by [`super::EventMsg`]
//! variants for in-process use, and (the same structs) are serialized as the
//! `params` field of a JSON-RPC notification by the server-side envelope union
//! `ServerNotification` (which stays in `slab-proto`).
//!
//! The approval params reference `slab_exec_policy::{OperationCategory,
//! ApprovalScope}` — wire-byte-identical to the `slab-proto` mirrors (both
//! `#[serde(rename_all = "snake_case")]`, same variants).

use serde::{Deserialize, Serialize};

use super::item::TurnItem;
use super::thread::Thread;
use super::turn::Turn;
use slab_types::ConversationMessage;

// ---- lifecycle ----

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStartedParams {
    pub thread: Thread,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartedParams {
    pub thread_id: String,
    pub turn: Turn,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TurnCompletedParams {
    pub thread_id: String,
    pub turn: Turn,
    /// Token usage for the turn (prompt / completion / total). `None` when the
    /// backend did not report usage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<TurnUsage>,
}

/// Token-usage snapshot reported at turn completion.
///
/// `prompt_tokens` reflects the full input context of the turn (including any
/// kv-cache-reused prefix reported as `cached_tokens`); `completion_tokens` is
/// the number of tokens generated.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TurnUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<u32>,
    #[serde(default)]
    pub estimated: bool,
}

impl From<crate::port::LlmUsage> for TurnUsage {
    fn from(usage: crate::port::LlmUsage) -> Self {
        let crate::port::LlmUsage { prompt_tokens, completion_tokens, total_tokens, estimated } =
            usage;
        Self { prompt_tokens, completion_tokens, total_tokens, cached_tokens: None, estimated }
    }
}

// ---- context compaction ----

/// Emitted when an auto-compaction summarization begins — after the policy's
/// threshold gate passes, before the summarization LLM call. The client shows an
/// in-progress "compacting context" indicator. Carries no `turn_id` so it
/// bypasses the client transport's turn-replay guard.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContextCompactingParams {
    pub thread_id: String,
}

/// Terminal compaction event. `status` is `"compacted"` on a successful replace
/// (with token/removed counts populated) or `"skipped"` when a started
/// compaction did not shrink the set — the client clears its in-progress
/// indicator in either case (no dangling "compacting" marker).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContextCompactedParams {
    pub thread_id: String,
    /// `"compacted"` (default when absent) or `"skipped"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed_messages: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u32>,
}

// ---- persistence-grade conversation events ----
//
// These two variants carry conversation data (a message append / a turn-state
// snapshot) out of slab-agent via the `EventMsg` protocol so the app-core
// rollout persistence observer can land it in the rollout true source — the
// sole conversation write path once the slab-agent `AgentStorePort` conversation
// methods are removed. They are NOT UI notifications (the projection maps them
// to `None`); they are persistence-only. slab-agent's existing `Turn*` lifecycle
// events carry no TurnState fields, so dedicated variants are required.

/// A conversation message was appended to a thread (user / injected /
/// assistant / tool-result). Carries the full `ThreadMessageRecord` shape so the
/// observer can build a `TurnContext::MessageAppend` rollout line preserving
/// F3 (the original record `id` + `created_at`).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MessageAppendedParams {
    pub thread_id: String,
    pub turn_index: u32,
    pub message: ConversationMessage,
    /// Original record id (F3). Carried through so replay recovers the React
    /// key instead of a synthetic `{thread_id}-r{seq}`.
    pub id: String,
    /// RFC 3339 creation timestamp (F3).
    pub created_at: String,
}

/// A turn-state snapshot was upserted (running / llm_completed / completed /
/// failed / ...). Carries the full `TurnStateRecord` shape so the observer can
/// build a `TurnContext::TurnState` rollout line. The input messages travel as
/// a typed vec (NOT a json blob), so the F6 raw-blob recovery path is dead for
/// this event — `input_messages_raw` is intentionally absent.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TurnStateChangedParams {
    pub thread_id: String,
    pub turn_index: u32,
    pub status: String,
    pub input_messages: Vec<ConversationMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_specs_json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_response_json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// RFC 3339 turn-start timestamp (F4).
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

// ---- items ----

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ItemStartedParams {
    pub item: TurnItem,
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ItemCompletedParams {
    pub item: TurnItem,
    pub thread_id: String,
    pub turn_id: String,
}

// ---- deltas ----

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageDeltaParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReasoningTextDeltaParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub content_index: u32,
    pub delta: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReasoningSummaryTextDeltaParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_index: Option<u32>,
    pub delta: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionOutputDeltaParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeOutputDeltaParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

// ---- approvals ----

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub command: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Operation category so the UI can render category-appropriate choices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<slab_exec_policy::OperationCategory>,
    /// Persistence scopes the client may offer (run-once / workspace / always
    /// / deny). Empty for servers that only support approve/reject.
    #[serde(default)]
    pub allowed_scopes: Vec<slab_exec_policy::ApprovalScope>,
    /// Full structured plan snapshot, present only on `present_plan` approvals
    /// so the UI can render a rich plan card (summary / counts / items /
    /// current step) instead of the one-line `command` text. Absent (and
    /// omitted from the wire) for all other approvals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_snapshot: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub changes: Vec<FileChangeApprovalChange>,
    #[serde(default)]
    pub allowed_scopes: Vec<slab_exec_policy::ApprovalScope>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeApprovalChange {
    pub path: String,
    /// Change kind, e.g. `add` / `edit` / `delete`.
    #[serde(rename = "type")]
    pub change_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}
