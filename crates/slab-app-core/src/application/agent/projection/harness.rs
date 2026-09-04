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
/// if any. `Error` and `TurnAborted` return `None` — the dispatcher
/// adapts them (error needs correlation ids; aborted maps to a `turn/completed`
/// with interrupted status). This free function replaces the old
/// `EventMsg::into_notification` method, which could not live on the slab-agent
/// semantic type without dragging in the wire-envelope crate.
pub fn event_msg_to_notification(msg: EventMsg) -> Option<ServerNotification> {
    match msg {
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
        EventMsg::BackgroundTaskUpdated(p) => Some(ServerNotification::BackgroundTaskUpdated(p)),
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
        EventMsg::Error(_) | EventMsg::TurnAborted(_) => None,
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
            stubbed_messages: None,
            output_tokens: Some(120),
        });
        let n = event_msg_to_notification(event).unwrap();
        assert_eq!(n.method(), "context/compacted");
    }

    #[test]
    fn background_task_event_lifts_to_notification() {
        let event =
            EventMsg::BackgroundTaskUpdated(slab_agent::protocol::BackgroundTaskUpdatedParams {
                thread_id: "t".to_owned(),
                task_id: "bg-1".to_owned(),
                status: "exited".to_owned(),
                kind: Some("shell".to_owned()),
                result_summary: None,
                exit_code: Some(0),
                pid: Some(4242),
                command: Some("npm run dev".to_owned()),
            });
        let n = event_msg_to_notification(event).unwrap();
        assert_eq!(n.method(), "backgroundTask/updated");
        let json = serde_json::to_value(&n).unwrap();
        assert_eq!(json["params"]["taskId"], "bg-1");
        assert_eq!(json["params"]["status"], "exited");
        assert_eq!(json["params"]["kind"], "shell");
        assert_eq!(json["params"]["pid"], 4242);
    }

    #[test]
    fn subagent_background_task_event_lifts_with_result_summary() {
        let event =
            EventMsg::BackgroundTaskUpdated(slab_agent::protocol::BackgroundTaskUpdatedParams {
                thread_id: "t".to_owned(),
                task_id: "bg-2".to_owned(),
                status: "completed".to_owned(),
                kind: Some("subagent".to_owned()),
                result_summary: Some("child result".to_owned()),
                exit_code: None,
                pid: None,
                command: Some("summarize the repo".to_owned()),
            });
        let n = event_msg_to_notification(event).unwrap();
        let json = serde_json::to_value(&n).unwrap();
        assert_eq!(json["params"]["kind"], "subagent");
        assert_eq!(json["params"]["resultSummary"], "child result");
        // Optional-absent fields must not leak onto the wire.
        assert!(json["params"].get("exitCode").is_none());
    }
}
