//! `Event` / `EventMsg` — the in-process event aggregate.
//!
//! One inbound slab-agent event projects to zero or more [`Event`]s. Each
//! `EventMsg` variant wraps the same payload struct used by the corresponding
//! JSON-RPC notification, so [`EventMsg::into_notification`] is a 1:1 lift
//! (except `Error`/`Warning`/`TurnAborted`, which the dispatcher adapts).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::harness::error::{ErrorEvent, WarningEvent};
use crate::harness::messages::Turn;
use crate::harness::notification::*;

/// A queued event from the agent, correlated with the submission `id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnAbortedParams {
    pub thread_id: String,
    pub turn: Turn,
}

/// Response event from the agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, strum::Display, JsonSchema)]
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

impl EventMsg {
    /// Convert into the matching wire notification, if any. `Error`, `Warning`,
    /// and `TurnAborted` return `None` — the dispatcher adapts them (error needs
    /// correlation ids; aborted maps to a `turn/completed` with interrupted
    /// status).
    pub fn into_notification(self) -> Option<ServerNotification> {
        Some(match self {
            Self::ThreadStarted(p) => ServerNotification::ThreadStarted(p),
            Self::TurnStarted(p) => ServerNotification::TurnStarted(p),
            Self::TurnCompleted(p) => ServerNotification::TurnCompleted(p),
            Self::ItemStarted(p) => ServerNotification::ItemStarted(p),
            Self::ItemCompleted(p) => ServerNotification::ItemCompleted(p),
            Self::AgentMessageDelta(p) => ServerNotification::AgentMessageDelta(p),
            Self::ReasoningTextDelta(p) => ServerNotification::ReasoningTextDelta(p),
            Self::ReasoningSummaryTextDelta(p) => ServerNotification::ReasoningSummaryTextDelta(p),
            Self::CommandExecutionOutputDelta(p) => {
                ServerNotification::CommandExecutionOutputDelta(p)
            }
            Self::FileChangeOutputDelta(p) => ServerNotification::FileChangeOutputDelta(p),
            Self::CommandExecutionRequestApproval(p) => {
                ServerNotification::CommandExecutionRequestApproval(p)
            }
            Self::FileChangeRequestApproval(p) => ServerNotification::FileChangeRequestApproval(p),
            Self::Error(_) | Self::Warning(_) | Self::TurnAborted(_) => return None,
        })
    }
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
    fn delta_event_lifts_to_notification() {
        let event = EventMsg::AgentMessageDelta(AgentMessageDeltaParams {
            thread_id: "t".to_owned(),
            turn_id: "tu".to_owned(),
            item_id: "i".to_owned(),
            delta: "x".to_owned(),
        });
        let n = event.into_notification().unwrap();
        assert_eq!(n.method(), "item/agentMessage/delta");
    }

    #[test]
    fn error_event_does_not_lift_to_notification() {
        let event = EventMsg::Error(ErrorEvent::new("boom"));
        assert!(event.into_notification().is_none());
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
