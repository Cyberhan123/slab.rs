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
use ts_rs::TS;

use super::item::TurnItem;
use super::turn::Turn;
use slab_types::ConversationMessage;

// ---- lifecycle ----

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TurnStartedParams {
    pub thread_id: String,
    pub turn: Turn,
}

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TurnCompletedParams {
    pub thread_id: String,
    pub turn: Turn,
    /// Token usage for the turn (prompt / completion / total). `None` when the
    /// backend did not report usage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<TurnUsage>,
    /// Why the run ended (`"completed"` on the normal path). Lets clients
    /// render a precise end state instead of guessing. Absent on events
    /// written by older servers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// `EventMsg::ThreadStatusChanged` params — projects the authoritative
/// thread-level status (`AgentThreadStatus`) to clients so the UI derives
/// busy/terminal state from the server instead of a local heuristic.
/// Previously `on_status_change` only logged and no thread status ever
/// reached the wire.
#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThreadStatusChangedParams {
    pub thread_id: String,
    /// `AgentThreadStatus` wire value: `pending` / `running` / `interrupting`
    /// / `interrupted` / `completed` / `errored` / `shutdown`.
    pub status: String,
    /// Optional detail (e.g. the termination reason persisted alongside the
    /// status).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Token-usage snapshot reported at turn completion.
///
/// `prompt_tokens` reflects the full input context of the turn (including any
/// kv-cache-reused prefix reported as `cached_tokens`); `completion_tokens` is
/// the number of tokens generated.
#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
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
        let crate::port::LlmUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cached_tokens,
            estimated,
        } = usage;
        Self { prompt_tokens, completion_tokens, total_tokens, cached_tokens, estimated }
    }
}

// ---- context compaction ----

/// Emitted when an auto-compaction summarization begins — after the policy's
/// threshold gate passes, before the summarization LLM call. The client shows an
/// in-progress "compacting context" indicator. Carries no `turn_id` so it
/// bypasses the client transport's turn-replay guard.
#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContextCompactingParams {
    pub thread_id: String,
}

/// Terminal compaction event. `status` is `"compacted"` on a successful replace
/// (with token/removed counts populated) or `"skipped"` when a started
/// compaction did not shrink the set — the client clears its in-progress
/// indicator in either case (no dangling "compacting" marker).
#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContextCompactedParams {
    pub thread_id: String,
    /// `"compacted"` (default when absent) or `"skipped"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed_messages: Option<u32>,
    /// Tool results condensed into structured stubs by the deterministic
    /// micro tier (content-only shrink; absent when no stubbing ran).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stubbed_messages: Option<u32>,
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

/// The typed payload of [`TurnStateChangedParams`] — what used to be three
/// semantics crammed into one flat struct (distinguished only by the
/// "`input_messages` is empty = no-op" CONVENTION) is now three variants:
///
/// - [`TurnStatePayload::Sampling`]: the sampling-entry full snapshot (the
///   per-iteration replay anchor). The ONLY variant that carries input —
///   replay replaces the conversation baseline with it.
/// - [`TurnStatePayload::Phase`]: an advisory phase announcement
///   (`executing_tools` / `awaiting_approval` / `compacting` / re-sampling).
///   No payload; the phase name travels on the params' `status` field.
/// - [`TurnStatePayload::Finished`]: the terminal turn result. Has NO input
///   field, so the historical "terminal line copies the whole input" bug is
///   structurally impossible.
///
/// The rollout disk line embeds this enum verbatim (`TurnContext::TurnState`
/// → `state`), so old flat lines (which have no `state` key) fail to parse
/// and are skipped by the reader's fault tolerance — old rollout files are
/// intentionally not compatible (they are disposable debug artifacts).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TurnStatePayload {
    /// Sampling-entry snapshot: the FULL message list the model was sent this
    /// iteration (replay anchor) plus the serialized tool specs.
    Sampling {
        input_messages: Vec<ConversationMessage>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_specs_json: Option<String>,
    },
    /// Advisory phase broadcast — status-only, no payload.
    Phase,
    /// Terminal turn result. No input field by construction.
    Finished {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        /// RFC 3339 completion timestamp.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        completed_at: Option<String>,
    },
}

impl TurnStatePayload {
    /// The input snapshot of a [`TurnStatePayload::Sampling`] variant, if this
    /// is one. Replay anchors read through here — a terminal/phase line
    /// cannot answer by construction.
    pub fn sampling_input(&self) -> Option<&[ConversationMessage]> {
        match self {
            Self::Sampling { input_messages, .. } => Some(input_messages),
            Self::Phase | Self::Finished { .. } => None,
        }
    }
}

/// A turn-state transition was recorded. The semantics live in
/// [`TurnStatePayload::state`]; `status` names the phase
/// (`TurnPhase::as_str`) for both advisory and terminal lines, and
/// `started_at` is the iteration's turn-start timestamp (F4).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TurnStateChangedParams {
    pub thread_id: String,
    pub turn_index: u32,
    pub status: String,
    pub state: TurnStatePayload,
    /// RFC 3339 turn-start timestamp (F4).
    pub started_at: String,
}

// ---- items ----

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ItemStartedParams {
    pub item: TurnItem,
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ItemCompletedParams {
    pub item: TurnItem,
    pub thread_id: String,
    pub turn_id: String,
}

// ---- deltas ----

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentMessageDeltaParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReasoningTextDeltaParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub content_index: u32,
    pub delta: String,
}

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReasoningSummaryTextDeltaParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_index: Option<u32>,
    pub delta: String,
}

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommandExecutionOutputDeltaParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileChangeOutputDeltaParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

/// `backgroundTask/updated` — a lifecycle transition of a resident background
/// task (started via `shell background=true` or `delegate_subagent`):
/// spawned/stopped from the registry, or terminated on its own
/// (exited/failed/completed).
#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackgroundTaskUpdatedParams {
    pub thread_id: String,
    pub task_id: String,
    /// `running` | `exited` | `stopped` | `failed` | `completed`.
    pub status: String,
    /// `shell` (default) | `subagent` — what kind of work the task tracks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Subagent tasks: truncated completion payload at the terminal event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

/// `subagent/childEvent` — a relayed child-agent turn item, re-published on
/// the PARENT channel by the app-core subagent bridge so harness clients can
/// render live child activity inside the delegate card. The inner item ids
/// stay child-scoped: they render card-locally and never enter the parent
/// thread's item registry, so no id namespacing is needed in v1.
#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SubagentChildEventParams {
    /// PARENT thread id (real id server-side; rewritten to the harness id on
    /// the wire like every other thread-keyed notification).
    pub thread_id: String,
    /// Child agent thread id (real; opaque correlation key client-side —
    /// matches the `child_thread_id` in the `delegate_subagent` tool output
    /// envelope).
    pub child_thread_id: String,
    /// `started` | `completed`.
    pub phase: String,
    /// The child turn the item belongs to (scopes `started`/`completed` pairs).
    pub turn_id: String,
    /// The child turn item (tool call / message), payload unchanged.
    pub item: TurnItem,
}

// ---- approvals ----

/// `item/questionnaire/requestAnswer` — the agent asked the user a structured
/// question (the `questionnaire` tool). The client renders the question card
/// and routes the user's answers back via the `questionnaire/resolve` request,
/// correlating on `item_id` (the tool-call correlation id, same key as the
/// approval notifications).
#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QuestionnaireRequestAnswerParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    /// The self-contained question text.
    pub question: String,
    /// The answer options (label + stable value + optional description).
    pub choices: Vec<QuestionnaireChoiceView>,
    /// Multiple choices may be selected.
    #[serde(default)]
    pub allow_multiple: bool,
    /// The user may type a custom answer instead of picking a choice.
    #[serde(default)]
    pub allow_custom_input: bool,
    /// The UI should nag for an answer (informational; a timeout is still a
    /// valid outcome the model handles).
    #[serde(default)]
    pub required: bool,
}

/// One answer option of a questionnaire, as delivered on the wire.
#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QuestionnaireChoiceView {
    /// Short human-readable label shown as the option.
    pub label: String,
    /// Stable machine value for the choice (defaults to the label).
    pub value: String,
    /// Optional one-line explanation shown under the label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
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

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileChangeRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub changes: Vec<FileChangeApprovalChange>,
    #[serde(default)]
    pub allowed_scopes: Vec<slab_exec_policy::ApprovalScope>,
}

#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileChangeApprovalChange {
    pub path: String,
    /// Change kind, e.g. `add` / `edit` / `delete`.
    #[serde(rename = "type")]
    pub change_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The port-level usage carries cache-hit counts (engine kv-cache reuse /
    /// provider prompt-cache hits); the wire conversion must not drop them —
    /// `cachedTokens` on `TurnCompleted` was permanently `None` before.
    #[test]
    fn turn_usage_carries_cached_tokens() {
        let usage = crate::port::LlmUsage {
            prompt_tokens: 1024,
            completion_tokens: 8,
            total_tokens: 1032,
            cached_tokens: Some(512),
            estimated: false,
        };
        assert_eq!(TurnUsage::from(usage).cached_tokens, Some(512));

        let bare = crate::port::LlmUsage {
            prompt_tokens: 10,
            completion_tokens: 1,
            total_tokens: 11,
            cached_tokens: None,
            estimated: true,
        };
        assert_eq!(TurnUsage::from(bare).cached_tokens, None);
    }

    // Design-refactor pin: the terminal turn-state payload is a typed
    // `Finished` variant with NO input field — the historical "completed line
    // copies the whole input" bug is structurally impossible. The rollout disk
    // line embeds this enum, so the wire shape here is the disk shape.
    #[test]
    fn turn_state_payload_discriminates_sampling_phase_finished() {
        let finished = TurnStateChangedParams {
            thread_id: "t".to_owned(),
            turn_index: 3,
            status: "completed".to_owned(),
            state: TurnStatePayload::Finished {
                error: None,
                completed_at: Some("2026-01-01T00:00:05Z".to_owned()),
            },
            started_at: "2026-01-01T00:00:00Z".to_owned(),
        };
        let json = serde_json::to_value(&finished).unwrap();
        assert_eq!(json["state"]["kind"], "finished");
        assert_eq!(json["state"]["completedAt"], "2026-01-01T00:00:05Z");
        assert!(
            json["state"].get("inputMessages").is_none() && json.get("inputMessages").is_none(),
            "terminal payload must carry no input snapshot: {json}"
        );

        let sampling = TurnStateChangedParams {
            thread_id: "t".to_owned(),
            turn_index: 3,
            status: "sampling".to_owned(),
            state: TurnStatePayload::Sampling {
                input_messages: vec![ConversationMessage {
                    role: "user".to_owned(),
                    content: slab_types::ConversationMessageContent::Text("hi".to_owned()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                }],
                tool_specs_json: None,
            },
            started_at: "2026-01-01T00:00:00Z".to_owned(),
        };
        let json = serde_json::to_value(&sampling).unwrap();
        assert_eq!(json["state"]["kind"], "sampling");
        assert_eq!(json["state"]["inputMessages"][0]["role"], "user");

        let phase = TurnStateChangedParams {
            thread_id: "t".to_owned(),
            turn_index: 3,
            status: "executing_tools".to_owned(),
            state: TurnStatePayload::Phase,
            started_at: "2026-01-01T00:00:00Z".to_owned(),
        };
        let json = serde_json::to_value(&phase).unwrap();
        assert_eq!(json["state"]["kind"], "phase");

        // sampling_input() is the typed replay gate: Phase/Finished cannot
        // answer, only Sampling does.
        assert!(TurnStatePayload::Phase.sampling_input().is_none());
        assert!(finished.state.sampling_input().is_none());
        assert_eq!(sampling.state.sampling_input().unwrap().len(), 1);
    }
}
