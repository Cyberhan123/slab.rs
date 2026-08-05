//! Core rollout line types.
//!
//! Every line in a `<thread_id>.rollout.jsonl` file is a [`RolloutLine`] wrapping
//! a [`RolloutItem`]. [`RolloutItem`] is **adjacently tagged** (`rolloutType` +
//! `item`) so its discriminator never collides with the inner discriminators of
//! [`TurnItem`] (`type`, camelCase) or [`EventMsg`] (`type`, snake_case) when
//! either is nested inside a rollout line.

use serde::{Deserialize, Serialize};

use slab_agent::protocol::{EventMsg, TurnItem};
use slab_types::ConversationMessage;

/// One record written to / read from a rollout JSONL file.
///
/// The adjacent tag (`rolloutType` + `item`) is deliberate: [`TurnItem`] and
/// [`EventMsg`] both carry their own inner `type` discriminator, so reusing
/// `type` here would alias and break round-trips.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "rolloutType", content = "item", rename_all = "camelCase")]
pub enum RolloutItem {
    /// First line of every file — describes the session.
    SessionMeta(SessionMeta),
    /// Full-fidelity UI artifact from `ItemCompleted`.
    TurnItem(TurnItem),
    /// Turn lifecycle + error/warning events (filtered by [`crate::policy`]).
    EventMsg(EventMsg),
    /// Context-compaction snapshot; on read, prior `TurnContext`/`TurnItem`
    /// messages are discarded and `compacted_messages` becomes the new baseline.
    Compacted(CompactedPayload),
    /// LLM-grade conversation deltas (`ConversationMessage` form).
    TurnContext(TurnContextPayload),
}

impl RolloutItem {
    /// A best-effort turn index carried by this item, if any.
    ///
    /// `TurnContext` variants and [`CompactedPayload`] carry their own
    /// `turn_index`. Other variants carry no turn affiliation (`None`); the
    /// store replay attaches the currently-tracked turn index when materializing
    /// records.
    pub fn turn_index(&self) -> Option<u32> {
        match self {
            Self::TurnContext(TurnContextPayload::TurnState { turn_index, .. })
            | Self::TurnContext(TurnContextPayload::MessageAppend { turn_index, .. }) => {
                Some(*turn_index)
            }
            Self::Compacted(payload) => Some(payload.turn_index),
            Self::SessionMeta(_) | Self::TurnItem(_) | Self::EventMsg(_) => None,
        }
    }
}

/// A single JSONL line: an RFC-3339 timestamp with a flattened [`RolloutItem`].
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct RolloutLine {
    /// RFC-3339 creation timestamp.
    pub timestamp: String,
    /// The payload — flattened so `rolloutType`/`item` sit beside `timestamp`.
    #[serde(flatten)]
    pub item: RolloutItem,
}

impl RolloutLine {
    /// Build a new line stamped with the current UTC time.
    pub fn now(item: RolloutItem) -> Self {
        Self { timestamp: chrono::Utc::now().to_rfc3339(), item }
    }

    /// Build a line with an explicit timestamp (useful for tests / replay).
    pub fn with_timestamp(timestamp: impl Into<String>, item: RolloutItem) -> Self {
        Self { timestamp: timestamp.into(), item }
    }
}

/// Session header — the first line of a rollout file.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    /// Owning thread id; matches the file name.
    pub thread_id: String,
    /// Session the thread belongs to.
    pub session_id: String,
    /// Parent thread if this is a fork/branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// RFC-3339 session start.
    pub started_at: String,
    /// Opaque agent config blob captured at session creation.
    pub config_json: serde_json::Value,
    /// On-disk format version; bumped on breaking line-format changes.
    pub rollout_version: u32,
    /// Optional agent role name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_name: Option<String>,
    /// Optional pointer to the Part-2 trace bundle directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_path: Option<String>,
}

impl SessionMeta {
    /// The rollout format version produced by this build.
    pub const CURRENT_VERSION: u32 = 1;
}

/// Payload of [`RolloutItem::Compacted`].
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompactedPayload {
    /// Owning thread id.
    pub thread_id: String,
    /// Post-compaction conversation baseline. May be empty when the summary is
    /// produced asynchronously (a later [`TurnContextPayload::TurnState`] fills it).
    pub compacted_messages: Vec<ConversationMessage>,
    /// Number of messages removed by this compaction.
    pub removed_messages: u32,
    /// Token count of the summary, when known.
    pub output_tokens: u32,
    /// Free-form compaction status (e.g. `"auto"`, `"manual"`).
    pub status: String,
    /// The turn at/after which this compaction took effect. A truncation that
    /// drops this turn must also drop the compaction marker (otherwise
    /// `crate::store::read_messages` would reset to a summary of messages that
    /// were rolled back). Defaults to `0` for older rollout files so they
    /// deserialize cleanly.
    #[serde(default)]
    pub turn_index: u32,
}

/// Payload of [`RolloutItem::TurnContext`] — the LLM-grade conversation deltas.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TurnContextPayload {
    /// A complete turn snapshot — what the model was sent and what it returned.
    TurnState {
        turn_index: u32,
        status: String,
        input_messages: Vec<ConversationMessage>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_specs_json: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        llm_response_json: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        completed_at: Option<String>,
        /// Original turn-start timestamp carried through from
        /// `TurnStateRecord::started_at` (F4). `None` on rollout files written
        /// before this field existed; replay falls back to the line timestamp.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        started_at: Option<String>,
        /// Raw `input_messages` JSON blob preserved verbatim when the typed
        /// `input_messages` list could not be parsed (F6). The replay returns
        /// this string directly so a malformed blob is recoverable instead of
        /// being silently emptied. `None` on the happy path (parsed list used).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input_messages_raw: Option<String>,
    },
    /// A single appended message (user input or a tool result) for `turn_index`.
    MessageAppend {
        turn_index: u32,
        message: ConversationMessage,
        /// Original `ThreadMessageRecord` id carried through verbatim (F3).
        /// `None` on rollout files written before this field existed; replay
        /// synthesizes `"{thread_id}-r{seq}"` for backward-compat.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Original message creation timestamp carried through from
        /// `ThreadMessageRecord::created_at` (F3). `None` on old files; replay
        /// falls back to the line timestamp.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        created_at: Option<String>,
    },
}

impl TurnContextPayload {
    /// The turn index this payload belongs to.
    pub fn turn_index(&self) -> u32 {
        match self {
            Self::TurnState { turn_index, .. } | Self::MessageAppend { turn_index, .. } => {
                *turn_index
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_agent::protocol::item::ReasoningText;
    use slab_agent::protocol::{AgentMessageDeltaParams, EventMsg};

    fn sample_turn_item() -> TurnItem {
        TurnItem::AgentMessage { id: "i-1".to_owned(), text: "hello".to_owned() }
    }

    #[test]
    fn rollout_item_is_adjacently_tagged() {
        let line = RolloutLine::with_timestamp(
            "2026-08-02T00:00:00Z",
            RolloutItem::TurnItem(sample_turn_item()),
        );
        let json = serde_json::to_value(&line).unwrap();
        // Outer discriminator is rolloutType + item.
        assert_eq!(json["rolloutType"], "turnItem");
        assert_eq!(json["timestamp"], "2026-08-02T00:00:00Z");
        // Inner TurnItem keeps its own camelCase `type` discriminator — no collision.
        assert_eq!(json["item"]["type"], "agentMessage");
        assert_eq!(json["item"]["id"], "i-1");
        assert_eq!(json["item"]["text"], "hello");
    }

    #[test]
    fn every_variant_round_trips() {
        let meta = SessionMeta {
            thread_id: "t1".to_owned(),
            session_id: "s1".to_owned(),
            parent_id: None,
            started_at: "2026-08-02T00:00:00Z".to_owned(),
            config_json: serde_json::json!({"model": "gpt"}),
            rollout_version: SessionMeta::CURRENT_VERSION,
            role_name: None,
            trace_path: None,
        };

        let cases: Vec<RolloutItem> = vec![
            RolloutItem::SessionMeta(meta.clone()),
            RolloutItem::TurnItem(TurnItem::UserMessage { id: "u1".to_owned(), content: vec![] }),
            RolloutItem::TurnItem(sample_turn_item()),
            RolloutItem::TurnItem(TurnItem::Reasoning {
                id: "r1".to_owned(),
                summary: ReasoningText::one("s"),
                content: ReasoningText::one("c"),
            }),
            RolloutItem::TurnItem(TurnItem::CommandExecution {
                id: "c1".to_owned(),
                command: "ls".to_owned(),
                cwd: "/tmp".to_owned(),
                process_id: None,
                status: "completed".to_owned(),
                aggregated_output: None,
                exit_code: Some(0),
                duration_ms: None,
            }),
            RolloutItem::TurnItem(TurnItem::FileChange {
                id: "f1".to_owned(),
                changes: vec![serde_json::json!({"path": "a.txt"})],
                status: "completed".to_owned(),
            }),
            RolloutItem::TurnItem(TurnItem::McpToolCall {
                id: "m1".to_owned(),
                server: "srv".to_owned(),
                tool: "t".to_owned(),
                arguments: serde_json::json!({}),
                status: "completed".to_owned(),
                result: None,
                error: None,
                duration_ms: None,
            }),
            RolloutItem::TurnItem(TurnItem::WebSearch {
                id: "w1".to_owned(),
                query: "rust".to_owned(),
            }),
            RolloutItem::TurnItem(TurnItem::ImageView {
                id: "iv1".to_owned(),
                path: "/p.png".to_owned(),
            }),
            RolloutItem::Compacted(CompactedPayload {
                thread_id: "t1".to_owned(),
                compacted_messages: vec![],
                removed_messages: 3,
                output_tokens: 10,
                status: "auto".to_owned(),
                turn_index: 1,
            }),
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index: 2,
                message: ConversationMessage {
                    role: "user".to_owned(),
                    content: slab_types::ConversationMessageContent::Text("hi".to_owned()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                id: None,
                created_at: None,
            }),
            // L1: exercise the snake_case inner `type` tag (EventMsg) against the
            // outer camelCase rolloutType — the no-collision pillar.
            RolloutItem::EventMsg(EventMsg::AgentMessageDelta(AgentMessageDeltaParams {
                thread_id: "t1".to_owned(),
                turn_id: "tu".to_owned(),
                item_id: "i".to_owned(),
                delta: "d".to_owned(),
            })),
            // L1: a TurnState round-trip (carries input_messages + turn_index).
            RolloutItem::TurnContext(TurnContextPayload::TurnState {
                turn_index: 1,
                status: "ok".to_owned(),
                input_messages: vec![],
                tool_specs_json: None,
                llm_response_json: None,
                error: None,
                completed_at: None,
                started_at: None,
                input_messages_raw: None,
            }),
        ];

        for item in cases {
            let line = RolloutLine::with_timestamp("2026-08-02T00:00:00Z", item.clone());
            let s = serde_json::to_string(&line).unwrap();
            let back: RolloutLine = serde_json::from_str(&s).unwrap();
            assert_eq!(back.timestamp, "2026-08-02T00:00:00Z");
            assert_eq!(back.item, item, "round-trip failed for: {s}");
        }
    }

    #[test]
    fn nested_turn_item_type_does_not_collide_with_rollout_type() {
        // A serialized TurnItem line carries BOTH "type":"agentMessage" (inner)
        // and "rolloutType":"turnItem" (outer). Parsing must recover both.
        let line = RolloutLine::with_timestamp("t", RolloutItem::TurnItem(sample_turn_item()));
        let s = serde_json::to_string(&line).unwrap();
        let parsed: RolloutLine = serde_json::from_str(&s).unwrap();
        let RolloutItem::TurnItem(ti) = parsed.item else {
            panic!("expected TurnItem variant");
        };
        assert_eq!(ti.id(), "i-1");
    }

    #[test]
    fn turn_context_tag_is_kind_camel_case() {
        let payload = TurnContextPayload::TurnState {
            turn_index: 1,
            status: "running".to_owned(),
            input_messages: vec![],
            tool_specs_json: None,
            llm_response_json: None,
            error: None,
            completed_at: None,
            started_at: None,
            input_messages_raw: None,
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["kind"], "turnState");
    }

    #[test]
    fn turn_index_helper() {
        let ts = TurnContextPayload::TurnState {
            turn_index: 5,
            status: "ok".to_owned(),
            input_messages: vec![],
            tool_specs_json: None,
            llm_response_json: None,
            error: None,
            completed_at: None,
            started_at: None,
            input_messages_raw: None,
        };
        assert_eq!(ts.turn_index(), 5);
        assert_eq!(RolloutItem::TurnContext(ts).turn_index(), Some(5));
        assert_eq!(RolloutItem::TurnItem(sample_turn_item()).turn_index(), None);
    }
}
