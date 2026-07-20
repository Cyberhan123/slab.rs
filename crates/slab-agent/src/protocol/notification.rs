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
