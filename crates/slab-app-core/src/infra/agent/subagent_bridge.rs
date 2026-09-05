//! Subagent lifecycle bridge: attaches rollout persistence to spawned child
//! threads and delivers background-delegation completions back to the parent
//! agent as a follow-up message (queue-or-resume).
//!
//! The bridge is injected into `DelegateSubagentTool` (as its
//! [`SubagentTaskSink`]) at control-construction time — BEFORE the
//! [`AgentCore`] that performs the delivery exists. The core is therefore
//! late-bound via [`OnceLock`] (same pattern as the memory pipeline's
//! `set_control`).

use std::collections::VecDeque;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use slab_agent::protocol::{
    EventMsg, ItemCompletedParams, ItemStartedParams, SubagentChildEventParams,
};
use slab_agent_tools::{
    BackgroundTaskStatus, SubagentFinishedEvent, SubagentSpawnedEvent, SubagentTaskSink,
};
use slab_types::{ConversationMessage, ConversationMessageContent};
use tokio::sync::broadcast;

use crate::domain::services::agent::AgentCore;
use crate::infra::agent::event_hub::AgentEventHub;

/// Cap the completion text embedded in the parent notification. Generous by
/// design (the parent agent consumes the result directly), but bounded so a
/// runaway child cannot flood the parent's context.
const MAX_NOTIFICATION_RESULT_CHARS: usize = 8_000;

/// Default stall threshold before a silent background delegation is reported
/// to the parent (notice-only, one-shot). Generous on purpose: a silent long
/// tool call (e.g. a plain `sleep` with no output) emits no events and would
/// false-positive under a tighter window — see [`SubagentWatchdog`].
const SUBAGENT_STALL_WARN_AFTER: Duration = Duration::from_secs(5 * 60);

pub(crate) struct SubagentBridge {
    core: OnceLock<Arc<AgentCore>>,
}

impl SubagentBridge {
    pub(crate) fn new() -> Self {
        Self { core: OnceLock::new() }
    }

    /// Late-bound because the tool router (carrying this bridge) is wired
    /// into `AgentControl` before the `AgentCore` wrapping that control
    /// exists. Binding twice is a bug — the second call is dropped with a warn.
    pub(crate) fn set_core(&self, core: Arc<AgentCore>) {
        if self.core.set(core).is_err() {
            tracing::warn!("subagent bridge core bound twice; keeping the first binding");
        }
    }

    fn core(&self) -> Option<&Arc<AgentCore>> {
        self.core.get()
    }
}

impl SubagentTaskSink for SubagentBridge {
    fn on_subagent_spawned(&self, event: SubagentSpawnedEvent) {
        let Some(core) = self.core() else {
            tracing::warn!(
                child_thread_id = event.child_thread_id,
                "subagent spawned before the bridge core was bound; child rollout persistence skipped"
            );
            return;
        };
        // Child threads are spawned inside the tool (slab-agent-tools cannot
        // reach app-core) — this is the ONLY attach point for their rollout
        // persistence observer. The hub's persistence replay buffer drains
        // atomically on subscribe, so no child event is lost.
        core.ensure_rollout_persistence(&event.child_thread_id);

        // Relay the child's turn items onto the parent's UI channel so harness
        // clients can render live child activity inside the delegate card.
        tokio::spawn(relay_child_events(
            Arc::clone(core.events()),
            event.child_thread_id.clone(),
            event.parent_thread_id.clone(),
        ));

        // One-shot stall watchdog. `no_resume` delegations asked NOT to be
        // woken by the child — a stall notice would violate that, so skip it.
        if !event.no_resume {
            let hub = Arc::clone(core.events());
            let child_thread_id = event.child_thread_id.clone();
            let notify_core = Arc::clone(core);
            let notice_parent = event.parent_thread_id.clone();
            tokio::spawn(SubagentWatchdog { stall_after: SUBAGENT_STALL_WARN_AFTER }.run(
                hub,
                child_thread_id,
                move |notice| {
                    tokio::spawn(async move {
                        // Same fence discipline as the completion path: the
                        // parent's tail must be durable before the notice
                        // re-reads the rollout.
                        notify_core.await_durable(&notice_parent).await;
                        let message = ConversationMessage {
                            role: "user".to_owned(),
                            content: ConversationMessageContent::Text(notice),
                            name: None,
                            tool_call_id: None,
                            tool_calls: Vec::new(),
                        };
                        if let Err(error) =
                            notify_core.send_input_message(&notice_parent, message).await
                        {
                            tracing::warn!(
                                %error,
                                parent_thread_id = %notice_parent,
                                "failed to deliver subagent stall notice to the parent"
                            );
                        }
                    });
                },
            ));
        }
    }

    fn on_subagent_finished(&self, event: SubagentFinishedEvent) {
        let Some(core) = self.core() else {
            tracing::warn!(
                parent_thread_id = event.parent_thread_id,
                "subagent finished before the bridge core was bound; parent not notified"
            );
            return;
        };
        if !should_notify_parent(&event) {
            tracing::debug!(task_id = %event.task_id, "no_resume delegation; parent not resumed");
            return;
        }
        let core = Arc::clone(core);
        tokio::spawn(async move {
            // Fence FIRST: the parent's own tail events (epilogue of the run
            // that delegated) must be durable before `send_input_message`
            // re-reads the rollout history, or the resume would rebuild a
            // tail-less conversation.
            core.await_durable(&event.parent_thread_id).await;
            let message = ConversationMessage {
                role: "user".to_owned(),
                content: ConversationMessageContent::Text(render_notification(&event)),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            };
            if let Err(error) = core.send_input_message(&event.parent_thread_id, message).await {
                // The parent may be archived/shut down — a missed follow-up
                // is unfortunate but not fatal; the registry result and the
                // artifact remain queryable.
                tracing::warn!(
                    %error,
                    parent_thread_id = event.parent_thread_id,
                    task_id = event.task_id,
                    "failed to deliver subagent completion to the parent"
                );
            }
        });
    }
}

/// Whether the finished delegation should deliver the parent follow-up
/// (`no_resume` delegations keep the result queryable via the registry /
/// the artifact but never wake the parent).
fn should_notify_parent(event: &SubagentFinishedEvent) -> bool {
    !event.no_resume
}

/// Relay the delegated child's turn items onto the PARENT thread's UI channel
/// so harness clients can render live child activity inside the delegate card.
///
/// Subscribes to the child's channel (replay + live), wraps every
/// `ItemStarted`/`ItemCompleted` into an [`EventMsg::SubagentChildEvent`]
/// keyed to the parent, and exits when the child channel closes or the child
/// reaches a terminal status. Only `item/started` + `item/completed` are
/// relayed — deltas and approvals stay child-local (deltas are chatty;
/// child approvals have no UI consumer today). The wrapped events are
/// broadcast UI-only ([`AgentEventHub::broadcast_event_msg`], never
/// re-persisted on the parent rollout — the child-side observer already
/// persists them on the child's own channel).
///
/// Replay window: `on_subagent_spawned` fires inside the delegate tool
/// execution (before `register_detached`), so events between the child spawn
/// and this subscribe live in the bounded replay buffer and are drained first.
async fn relay_child_events(
    hub: Arc<AgentEventHub>,
    child_thread_id: String,
    parent_thread_id: String,
) {
    let subscription = hub.subscribe_event_msgs(&child_thread_id);
    let mut replay: VecDeque<EventMsg> = subscription.replay.into_iter().map(|e| e.msg).collect();
    let mut receiver = subscription.receiver;

    loop {
        let msg = if let Some(msg) = replay.pop_front() {
            msg
        } else {
            match receiver.recv().await {
                Ok(envelope) => envelope.msg,
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    // UI-grade relay: the activity list is best-effort; keep
                    // the tail rather than tearing the relay down.
                    tracing::warn!(
                        child_thread_id = %child_thread_id,
                        skipped,
                        "subagent child relay lagged; dropped older child items"
                    );
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        };

        match msg {
            EventMsg::ItemStarted(p) => {
                let ItemStartedParams { item, turn_id, .. } = p;
                hub.broadcast_event_msg(
                    &parent_thread_id,
                    EventMsg::SubagentChildEvent(SubagentChildEventParams {
                        thread_id: parent_thread_id.clone(),
                        child_thread_id: child_thread_id.clone(),
                        phase: "started".to_owned(),
                        turn_id,
                        item,
                    }),
                );
            }
            EventMsg::ItemCompleted(p) => {
                let ItemCompletedParams { item, turn_id, .. } = p;
                hub.broadcast_event_msg(
                    &parent_thread_id,
                    EventMsg::SubagentChildEvent(SubagentChildEventParams {
                        thread_id: parent_thread_id.clone(),
                        child_thread_id: child_thread_id.clone(),
                        phase: "completed".to_owned(),
                        turn_id,
                        item,
                    }),
                );
            }
            EventMsg::ThreadStatusChanged(p) if is_terminal_thread_status(&p.status) => break,
            _ => {}
        }
    }
}

/// One-shot stall watchdog for a background delegation: notify the parent ONCE
/// when the child has produced no events for `stall_after` (notice-only — it
/// never interrupts the child). Runs until the child channel closes or the
/// child reaches a terminal status; every received event (a Lag counts too)
/// refreshes the activity watermark.
///
/// False-positive surface: a silent long-running tool (e.g. `sleep 300` with
/// no output deltas) emits no events, so a child may be reported stalled while
/// legitimately busy. The 5-minute default + notice-only + one-shot semantics
/// make that an acceptable v1 trade-off; a future refinement could refresh the
/// watermark while an `ItemStarted` is open.
pub(crate) struct SubagentWatchdog {
    stall_after: Duration,
}

impl SubagentWatchdog {
    pub(crate) async fn run<N>(self, hub: Arc<AgentEventHub>, child_thread_id: String, notify: N)
    where
        N: FnOnce(String) + Send + 'static,
    {
        let subscription = hub.subscribe_event_msgs(&child_thread_id);
        let mut receiver = subscription.receiver;
        let mut watermark = tokio::time::Instant::now();
        // `N` is `FnOnce` — `take()` makes the at-most-once call explicit.
        let mut notify = Some(notify);

        loop {
            let deadline = watermark + self.stall_after;
            match tokio::time::timeout_at(deadline, receiver.recv()).await {
                Ok(Ok(envelope)) => {
                    watermark = tokio::time::Instant::now();
                    if let EventMsg::ThreadStatusChanged(p) = envelope.msg
                        && is_terminal_thread_status(&p.status)
                    {
                        break;
                    }
                }
                Ok(Err(broadcast::error::RecvError::Lagged(_))) => {
                    // A lag proves the child IS producing events.
                    watermark = tokio::time::Instant::now();
                }
                Ok(Err(broadcast::error::RecvError::Closed)) => break,
                Err(_elapsed) => {
                    if let Some(notify) = notify.take() {
                        notify(render_stall_notice(&child_thread_id, self.stall_after));
                    }
                    // Re-arm the watermark so the idle loop wakes at most once
                    // per `stall_after` instead of spinning on elapsed deadlines.
                    watermark = tokio::time::Instant::now();
                }
            }
        }
    }
}

/// Render the stall notice. Deliberately NOT the `[subagent task finished]`
/// prefix — a stalled child has no result to hand over.
fn render_stall_notice(child_thread_id: &str, after: Duration) -> String {
    format!(
        "[subagent task stalled] child_thread_id={child_thread_id} no activity for {} minutes; it may be stuck on a long tool call. Use subagent_status to inspect or subagent_stop to cancel.",
        (after.as_secs() / 60).max(1)
    )
}

fn is_terminal_thread_status(status: &str) -> bool {
    matches!(status, "interrupted" | "completed" | "errored" | "shutdown")
}

/// Render the parent-facing notification message.
fn render_notification(event: &SubagentFinishedEvent) -> String {
    let mut text = format!(
        "[subagent task finished] task_id={} status={}",
        event.task_id,
        event.status.as_str()
    );
    if !event.task_summary.is_empty() {
        text.push_str("\nTask: ");
        text.push_str(&event.task_summary);
    }
    if event.status == BackgroundTaskStatus::Completed {
        text.push_str("\nThe delegated subagent finished; act on its result or continue the outstanding work.");
    }
    match (&event.completion_text, event.artifact_refs.as_slice()) {
        (Some(completion), _) if !completion.is_empty() => {
            text.push_str("\nResult: ");
            if completion.chars().count() > MAX_NOTIFICATION_RESULT_CHARS {
                let truncated: String =
                    completion.chars().take(MAX_NOTIFICATION_RESULT_CHARS).collect();
                text.push_str(&truncated);
                text.push_str("\n(result truncated — full text in the artifact)");
            } else {
                text.push_str(completion);
            }
        }
        (_, refs) if !refs.is_empty() => {
            text.push_str("\nResult artifact: ");
            text.push_str(&refs.join(", "));
        }
        _ => {}
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finished(completion: Option<&str>, refs: &[&str]) -> SubagentFinishedEvent {
        SubagentFinishedEvent {
            parent_thread_id: "parent".to_owned(),
            child_thread_id: "child".to_owned(),
            task_id: "bg-x-1".to_owned(),
            task_summary: "summarize the repo".to_owned(),
            status: BackgroundTaskStatus::Completed,
            completion_text: completion.map(str::to_owned),
            artifact_refs: refs.iter().map(|r| (*r).to_owned()).collect(),
            no_resume: false,
        }
    }

    #[test]
    fn notification_renders_completion_text() {
        let text = render_notification(&finished(Some("all good"), &[]));
        assert!(text.starts_with("[subagent task finished] task_id=bg-x-1 status=completed"));
        assert!(text.contains("Task: summarize the repo"));
        assert!(text.contains("Result: all good"));
    }

    #[test]
    fn notification_prefers_artifact_reference_when_text_spilled() {
        let text = render_notification(&finished(None, &[".slab/artifacts/child/result.json"]));
        assert!(text.contains("Result artifact: .slab/artifacts/child/result.json"));
        assert!(!text.contains("Result: \n"));
    }

    #[test]
    fn notification_truncates_runaway_results() {
        let long = "x".repeat(MAX_NOTIFICATION_RESULT_CHARS + 100);
        let text = render_notification(&finished(Some(&long), &[]));
        assert!(text.contains("(result truncated — full text in the artifact)"));
    }

    #[test]
    fn unbound_core_is_reported_not_panicked() {
        let bridge = SubagentBridge::new();
        bridge.on_subagent_spawned(SubagentSpawnedEvent {
            parent_thread_id: "p".to_owned(),
            child_thread_id: "c".to_owned(),
            no_resume: false,
        });
        bridge.on_subagent_finished(finished(Some("done"), &[]));
    }

    #[test]
    fn should_notify_parent_reflects_no_resume() {
        assert!(should_notify_parent(&finished(Some("done"), &[])));
        let mut suppressed = finished(Some("done"), &[]);
        suppressed.no_resume = true;
        assert!(!should_notify_parent(&suppressed));
    }

    #[test]
    fn no_resume_finished_event_returns_early_without_panic() {
        let bridge = SubagentBridge::new();
        let mut event = finished(Some("done"), &[]);
        event.no_resume = true;
        // Unbound core → the guard path; either way the no_resume event must
        // return without spawning delivery work or panicking.
        bridge.on_subagent_finished(event);
    }

    mod relay_and_watchdog {
        use std::time::Duration;

        use super::super::*;
        use slab_agent::protocol::{ThreadStatusChangedParams, TurnItem};
        use slab_types::AgentThreadStatus;

        const CHILD: &str = "child-1";
        const PARENT: &str = "parent-1";

        fn tool_item(id: &str, status: &str) -> TurnItem {
            TurnItem::ToolCall {
                id: id.to_owned(),
                tool: "read_file".to_owned(),
                arguments: serde_json::json!({ "path": "README.md" }),
                status: status.to_owned(),
                result: None,
                error: None,
                duration_ms: None,
            }
        }

        fn item_started(id: &str) -> EventMsg {
            EventMsg::ItemStarted(ItemStartedParams {
                thread_id: CHILD.to_owned(),
                turn_id: "tu-1".to_owned(),
                item: tool_item(id, "in_progress"),
            })
        }

        fn item_completed(id: &str) -> EventMsg {
            EventMsg::ItemCompleted(ItemCompletedParams {
                thread_id: CHILD.to_owned(),
                turn_id: "tu-1".to_owned(),
                item: tool_item(id, "completed"),
            })
        }

        fn thread_status(status: AgentThreadStatus) -> EventMsg {
            EventMsg::ThreadStatusChanged(ThreadStatusChangedParams {
                thread_id: CHILD.to_owned(),
                status: status.to_string(),
                reason: None,
            })
        }

        fn child_tool_call_item() -> TurnItem {
            tool_item("item-1", "in_progress")
        }

        /// Items emitted BEFORE the relay subscribes must arrive via the
        /// replay drain; the terminal status (live) must end the relay.
        #[tokio::test]
        async fn relay_child_events_wraps_items_and_exits_on_terminal() {
            let hub = Arc::new(AgentEventHub::new());
            let mut parent_sub = hub.subscribe_event_msgs(PARENT);

            // Replay window: emitted before the relay task subscribes.
            hub.broadcast_event_msg(CHILD, item_started("item-1"));
            hub.broadcast_event_msg(CHILD, item_completed("item-1"));

            let relay = tokio::spawn(relay_child_events(
                Arc::clone(&hub),
                CHILD.to_owned(),
                PARENT.to_owned(),
            ));
            // Let the relay subscribe before the terminal goes out (it would
            // still reach the relay via replay, but this exercises live).
            tokio::time::sleep(Duration::from_millis(50)).await;
            hub.broadcast_event_msg(CHILD, thread_status(AgentThreadStatus::Completed));

            let first = tokio::time::timeout(Duration::from_secs(2), parent_sub.receiver.recv())
                .await
                .expect("first relayed event within timeout")
                .expect("parent channel open");
            let second = tokio::time::timeout(Duration::from_secs(2), parent_sub.receiver.recv())
                .await
                .expect("second relayed event within timeout")
                .expect("parent channel open");

            let EventMsg::SubagentChildEvent(p) = first.msg else {
                panic!("expected a relayed SubagentChildEvent");
            };
            assert_eq!(p.thread_id, PARENT);
            assert_eq!(p.child_thread_id, CHILD);
            assert_eq!(p.phase, "started");
            assert_eq!(p.turn_id, "tu-1");
            assert_eq!(p.item, child_tool_call_item());

            let EventMsg::SubagentChildEvent(p) = second.msg else {
                panic!("expected a relayed SubagentChildEvent");
            };
            assert_eq!(p.phase, "completed");
            assert_eq!(p.item, tool_item("item-1", "completed"));

            // Exactly two events relayed and the task finished.
            tokio::time::timeout(Duration::from_secs(2), relay)
                .await
                .expect("relay exits on terminal status")
                .expect("relay task ok");
            assert!(
                tokio::time::timeout(Duration::from_millis(100), parent_sub.receiver.recv())
                    .await
                    .is_err()
                    || parent_sub.receiver.is_empty(),
                "no further relayed events"
            );
        }

        #[tokio::test]
        async fn watchdog_fires_once_when_child_silent_then_exits_on_terminal() {
            let hub = Arc::new(AgentEventHub::new());
            let notices: Arc<std::sync::Mutex<Vec<String>>> = Arc::default();
            let recorder = Arc::clone(&notices);
            let watchdog =
                tokio::spawn(SubagentWatchdog { stall_after: Duration::from_millis(100) }.run(
                    Arc::clone(&hub),
                    CHILD.to_owned(),
                    move |notice| {
                        recorder.lock().unwrap_or_else(|p| p.into_inner()).push(notice);
                    },
                ));

            // No child events → exactly one notice within a generous window.
            let observed = tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if !notices.lock().unwrap_or_else(|p| p.into_inner()).is_empty() {
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await;
            assert!(observed.is_ok(), "stall notice fired");

            // Terminal ends the watchdog; the notice count stays at one.
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while !watchdog.is_finished() && std::time::Instant::now() < deadline {
                hub.broadcast_event_msg(CHILD, thread_status(AgentThreadStatus::Completed));
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            tokio::time::timeout(Duration::from_secs(2), watchdog)
                .await
                .expect("watchdog exits on terminal")
                .expect("watchdog task ok");
            let count = notices.lock().unwrap_or_else(|p| p.into_inner()).len();
            assert_eq!(count, 1, "stall notice is one-shot");
        }

        #[tokio::test]
        async fn watchdog_stays_quiet_while_child_is_active() {
            let hub = Arc::new(AgentEventHub::new());
            let notices: Arc<std::sync::Mutex<Vec<String>>> = Arc::default();
            let recorder = Arc::clone(&notices);
            let watchdog =
                tokio::spawn(SubagentWatchdog { stall_after: Duration::from_millis(150) }.run(
                    Arc::clone(&hub),
                    CHILD.to_owned(),
                    move |notice| {
                        recorder.lock().unwrap_or_else(|p| p.into_inner()).push(notice);
                    },
                ));
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Activity every 50ms across three stall windows: no notice.
            for index in 0..6 {
                hub.broadcast_event_msg(CHILD, item_started(&format!("item-{index}")));
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            assert!(
                notices.lock().unwrap_or_else(|p| p.into_inner()).is_empty(),
                "active child must not be reported stalled"
            );

            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while !watchdog.is_finished() && std::time::Instant::now() < deadline {
                hub.broadcast_event_msg(CHILD, thread_status(AgentThreadStatus::Shutdown));
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            tokio::time::timeout(Duration::from_secs(2), watchdog)
                .await
                .expect("watchdog exits on terminal")
                .expect("watchdog task ok");
        }

        #[tokio::test]
        async fn stall_notice_mentions_the_child_and_never_claims_finished() {
            let notice = render_stall_notice(CHILD, Duration::from_secs(300));
            assert!(notice.starts_with("[subagent task stalled]"));
            assert!(notice.contains(CHILD));
            assert!(!notice.contains("[subagent task finished]"));
        }
    }
}
