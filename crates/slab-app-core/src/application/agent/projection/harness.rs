//! Harness event lift — convert the slab-agent harness protocol (`EventMsg`)
//! into the JSON-RPC wire (`ServerNotification`).
//!
//! slab-agent emits the harness protocol directly (the `EventMsg`/`TurnItem`
//! surface in `slab-agent::protocol`); the legacy `AgentEventKind` stream feeds
//! only `/responses`. This module is the pure lift from an [`EventMsg`] to its
//! wire [`ServerNotification`] — there is no longer a stateful projection from
//! `AgentEventKind` (that was retired once slab-agent started emitting
//! `EventMsg` for turn lifecycle, text, reasoning, tool items, and approvals).
//!
//! Boundary: pure conversion only. No `tokio`, `axum`, or agent-service calls.

use slab_agent::protocol::EventMsg;
use slab_proto::harness::notification::ServerNotification;

/// Lifting helper: convert an [`EventMsg`] into its wire [`ServerNotification`],
/// if any. `Error`, `Warning`, and `TurnAborted` return `None` — the dispatcher
/// adapts them (error needs correlation ids; aborted maps to a `turn/completed`
/// with interrupted status). This free function replaces the old
/// `EventMsg::into_notification` method, which could not live on the slab-agent
/// semantic type without dragging in the wire-envelope crate.
pub fn event_msg_to_notification(msg: EventMsg) -> Option<ServerNotification> {
    match msg {
        EventMsg::ThreadStarted(p) => Some(ServerNotification::ThreadStarted(p)),
        EventMsg::ThreadStatusChanged(p) => Some(ServerNotification::ThreadStatusChanged(p)),
        EventMsg::TurnStarted(p) => Some(ServerNotification::TurnStarted(p)),
        EventMsg::TurnCompleted(p) => Some(ServerNotification::TurnCompleted(p)),
        EventMsg::ItemStarted(p) => Some(ServerNotification::ItemStarted(p)),
        EventMsg::ItemCompleted(p) => Some(ServerNotification::ItemCompleted(p)),
        EventMsg::AgentMessageDelta(p) => Some(ServerNotification::AgentMessageDelta(p)),
        EventMsg::ReasoningTextDelta(p) => Some(ServerNotification::ReasoningTextDelta(p)),
        EventMsg::ReasoningSummaryTextDelta(p) => {
            Some(ServerNotification::ReasoningSummaryTextDelta(p))
        }
        EventMsg::CommandExecutionOutputDelta(p) => {
            Some(ServerNotification::CommandExecutionOutputDelta(p))
        }
        EventMsg::FileChangeOutputDelta(p) => Some(ServerNotification::FileChangeOutputDelta(p)),
        EventMsg::CommandExecutionRequestApproval(p) => {
            Some(ServerNotification::CommandExecutionRequestApproval(p))
        }
        EventMsg::FileChangeRequestApproval(p) => {
            Some(ServerNotification::FileChangeRequestApproval(p))
        }
        EventMsg::ContextCompacting(p) => Some(ServerNotification::ContextCompacting(p)),
        EventMsg::ContextCompacted(p) => Some(ServerNotification::ContextCompacted(p)),
        // Persistence-grade conversation events. They carry data for
        // the rollout observer only — no UI notification maps to them.
        EventMsg::MessageAppended(_) | EventMsg::TurnStateChanged(_) => None,
        EventMsg::Error(_) | EventMsg::Warning(_) | EventMsg::TurnAborted(_) => None,
        // `EventMsg` is `#[non_exhaustive]`; future variants added in slab-agent
        // have no wire-notification mapping yet — drop them rather than failing
        // to compile the lift when slab-agent grows a new event.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_agent::protocol::{
        AgentMessageDeltaParams, ContextCompactedParams, ContextCompactingParams, ErrorEvent,
    };

    #[test]
    fn delta_event_lifts_to_notification() {
        let event = EventMsg::AgentMessageDelta(AgentMessageDeltaParams {
            thread_id: "t".to_owned(),
            turn_id: "tu".to_owned(),
            item_id: "i".to_owned(),
            delta: "x".to_owned(),
        });
        let n = event_msg_to_notification(event).unwrap();
        assert_eq!(n.method(), "item/agentMessage/delta");
    }

    #[test]
    fn error_event_does_not_lift_to_notification() {
        let event = EventMsg::Error(ErrorEvent::new("boom"));
        assert!(event_msg_to_notification(event).is_none());
    }

    #[test]
    fn context_compacting_event_lifts_to_notification() {
        let event =
            EventMsg::ContextCompacting(ContextCompactingParams { thread_id: "t".to_owned() });
        let n = event_msg_to_notification(event).unwrap();
        assert_eq!(n.method(), "context/compacting");
    }

    #[test]
    fn context_compacted_event_lifts_to_notification() {
        let event = EventMsg::ContextCompacted(ContextCompactedParams {
            thread_id: "t".to_owned(),
            status: Some("compacted".to_owned()),
            removed_messages: Some(3),
            output_tokens: Some(120),
        });
        let n = event_msg_to_notification(event).unwrap();
        assert_eq!(n.method(), "context/compacted");
    }
}
