//! `Event` / `EventMsg` — the in-process event aggregate.
//!
//! One inbound slab-agent event projects to zero or more [`Event`]s. Each
//! `EventMsg` variant wraps the same payload struct used by the corresponding
//! JSON-RPC notification. The 1:1 lift from `EventMsg` to the wire
//! `ServerNotification` lives in the server-side projection
//! (`event_msg_to_notification`), not here, so this module stays free of the
//! wire-envelope crate.

use serde::{Deserialize, Serialize};

use super::error::{ErrorEvent, WarningEvent};
use super::notification::*;
use super::turn::Turn;

/// A queued event from the agent, correlated with the submission `id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// Submission id (the originating JSON-RPC request id) this event belongs
    /// to. Lets a client multiplex concurrent turns on one socket.
    pub id: String,
    pub msg: EventMsg,
}

impl Event {
    pub fn new(id: impl Into<String>, msg: EventMsg) -> Self {
        Self { id: id.into(), msg }
    }
}

/// Internal params for an aborted turn (maps to a `turn/completed` notification
/// with an interrupted status on the wire).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TurnAbortedParams {
    pub thread_id: String,
    pub turn: Turn,
}

/// Response event from the agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, strum::Display)]
#[serde(tag = "type", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum EventMsg {
    Error(ErrorEvent),
    Warning(WarningEvent),

    ThreadStarted(ThreadStartedParams),
    TurnStarted(TurnStartedParams),
    TurnCompleted(TurnCompletedParams),
    TurnAborted(TurnAbortedParams),

    ItemStarted(ItemStartedParams),
    ItemCompleted(ItemCompletedParams),

    AgentMessageDelta(AgentMessageDeltaParams),
    ReasoningTextDelta(ReasoningTextDeltaParams),
    ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaParams),
    CommandExecutionOutputDelta(CommandExecutionOutputDeltaParams),
    FileChangeOutputDelta(FileChangeOutputDeltaParams),

    CommandExecutionRequestApproval(CommandExecutionRequestApprovalParams),
    FileChangeRequestApproval(FileChangeRequestApprovalParams),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_serializes_with_type_tag() {
        let event = Event::new(
            "req-1",
            EventMsg::AgentMessageDelta(AgentMessageDeltaParams {
                thread_id: "t1".to_owned(),
                turn_id: "tu1".to_owned(),
                item_id: "i1".to_owned(),
                delta: "hi".to_owned(),
            }),
        );
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["id"], "req-1");
        assert_eq!(json["msg"]["type"], "agent_message_delta");
        assert_eq!(json["msg"]["itemId"], "i1");
    }

    #[test]
    fn event_msg_displays_snake_case() {
        let event = EventMsg::AgentMessageDelta(AgentMessageDeltaParams {
            thread_id: "t".to_owned(),
            turn_id: "tu".to_owned(),
            item_id: "i".to_owned(),
            delta: "x".to_owned(),
        });
        assert_eq!(event.to_string(), "agent_message_delta");
    }
}
