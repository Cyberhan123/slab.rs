//! Subagent lifecycle bridge: attaches rollout persistence to spawned child
//! threads and delivers background-delegation completions back to the parent
//! agent as a follow-up message (queue-or-resume).
//!
//! The bridge is injected into `DelegateSubagentTool` (as its
//! [`SubagentTaskSink`]) at control-construction time — BEFORE the
//! [`AgentCore`] that performs the delivery exists. The core is therefore
//! late-bound via [`OnceLock`] (same pattern as the memory pipeline's
//! `set_control`).

use std::sync::{Arc, OnceLock};

use slab_agent_tools::{
    BackgroundTaskStatus, SubagentFinishedEvent, SubagentSpawnedEvent, SubagentTaskSink,
};
use slab_types::{ConversationMessage, ConversationMessageContent};

use crate::domain::services::agent::AgentCore;

/// Cap the completion text embedded in the parent notification. Generous by
/// design (the parent agent consumes the result directly), but bounded so a
/// runaway child cannot flood the parent's context.
const MAX_NOTIFICATION_RESULT_CHARS: usize = 8_000;

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
    }

    fn on_subagent_finished(&self, event: SubagentFinishedEvent) {
        let Some(core) = self.core() else {
            tracing::warn!(
                parent_thread_id = event.parent_thread_id,
                "subagent finished before the bridge core was bound; parent not notified"
            );
            return;
        };
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
        });
        bridge.on_subagent_finished(finished(Some("done"), &[]));
    }
}
