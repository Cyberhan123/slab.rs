//! `subagent_*` tools: communicate with background subagent delegations
//! started via `delegate_subagent` (default background mode).
//!
//! These are deliberately separate from the `task_*` family (background
//! SHELL processes): a subagent task tracks a child agent thread, accepts
//! steering messages while it runs, and reports a completion result instead
//! of an exit code.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{AgentControl, AgentError, SendOutcome, ToolContext, ToolHandler, ToolOutput};
use slab_types::{ConversationMessage, ConversationMessageContent};

use crate::background::{BackgroundTaskRegistry, BackgroundTaskSnapshot, TaskKind};

/// Fetch and kind-check a registered subagent task.
fn subagent_snapshot(
    registry: &BackgroundTaskRegistry,
    task_id: &str,
) -> Result<BackgroundTaskSnapshot, AgentError> {
    let snapshot = registry
        .snapshot(task_id)
        .ok_or_else(|| AgentError::ToolExecution(format!("unknown background task: {task_id}")))?;
    if snapshot.kind != TaskKind::Subagent {
        return Err(AgentError::ToolExecution(format!(
            "{task_id} is a {} task, not a subagent delegation (use the task_* tools for those)",
            snapshot.kind.as_str()
        )));
    }
    Ok(snapshot)
}

/// `subagent_status`: current state of one background subagent delegation
/// (or all of them when `task_id` is omitted).
pub struct SubagentStatusTool {
    registry: Arc<BackgroundTaskRegistry>,
}

impl SubagentStatusTool {
    pub fn new(registry: Arc<BackgroundTaskRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl ToolHandler for SubagentStatusTool {
    fn name(&self) -> &str {
        "subagent_status"
    }

    fn description(&self) -> &str {
        "Report the status of a background subagent started with \
         delegate_subagent (running/completed/errored/stopped, plus its \
         result once terminal), or list all subagent delegations when task_id \
         is omitted."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "Task id returned by delegate_subagent; omit to list all subagent delegations."
                }
            }
        })
    }

    /// Read-only registry query.
    fn is_concurrency_safe(&self, _arguments: &Value) -> bool {
        true
    }

    fn describe_operation(&self, arguments: &Value) -> Option<slab_agent::OperationDescriptor> {
        let task_id = arguments.get("task_id").and_then(Value::as_str)?;
        Some(slab_agent::OperationDescriptor::read_only(format!("subagent_status: {task_id}")))
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let content = match arguments.get("task_id").and_then(Value::as_str) {
            Some(task_id) => {
                let task = subagent_snapshot(&self.registry, task_id)?;
                serde_json::json!({ "task": subagent_snapshot_json(&task) })
            }
            None => serde_json::json!({
                "tasks": self
                    .registry
                    .list()
                    .iter()
                    .filter(|task| task.kind == TaskKind::Subagent)
                    .map(subagent_snapshot_json)
                    .collect::<Vec<_>>()
            }),
        };
        Ok(ToolOutput { content: content.to_string(), metadata: None })
    }
}

fn subagent_snapshot_json(task: &BackgroundTaskSnapshot) -> Value {
    serde_json::json!({
        "task_id": task.task_id,
        "parent_thread_id": task.thread_id,
        "child_thread_id": task.child_thread_id,
        "task": task.command,
        "status": task.status.as_str(),
        "result": task.result,
    })
}

/// `subagent_message`: queue a steering message into a RUNNING subagent.
pub struct SubagentMessageTool {
    registry: Arc<BackgroundTaskRegistry>,
    control: Arc<AgentControl>,
}

impl SubagentMessageTool {
    pub fn new(registry: Arc<BackgroundTaskRegistry>, control: Arc<AgentControl>) -> Self {
        Self { registry, control }
    }
}

#[async_trait]
impl ToolHandler for SubagentMessageTool {
    fn name(&self) -> &str {
        "subagent_message"
    }

    fn description(&self) -> &str {
        "Send a steering message to a RUNNING background subagent (e.g. extra \
         instructions, a scope change, or a request to wrap up). The message \
         is injected at the subagent's next iteration boundary — it does not \
         interrupt the step currently in flight."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "Task id returned by delegate_subagent."
                },
                "message": {
                    "type": "string",
                    "description": "Message text for the subagent."
                }
            },
            "required": ["task_id", "message"]
        })
    }

    /// Steers a specific child agent's input queue — keep it exclusive.
    fn is_concurrency_safe(&self, _arguments: &Value) -> bool {
        false
    }

    fn describe_operation(&self, arguments: &Value) -> Option<slab_agent::OperationDescriptor> {
        let task_id = arguments.get("task_id").and_then(Value::as_str)?;
        Some(slab_agent::OperationDescriptor::read_only(format!("subagent_message: {task_id}")))
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let task_id = arguments
            .get("task_id")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::ToolExecution("missing 'task_id' argument".into()))?;
        let message = arguments
            .get("message")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AgentError::ToolExecution("missing 'message' argument".into()))?;

        let task = subagent_snapshot(&self.registry, task_id)?;
        if task.status != crate::background::BackgroundTaskStatus::Running {
            return Ok(ToolOutput {
                content: serde_json::json!({
                    "queued": false,
                    "status": task.status.as_str(),
                    "note": "subagent is not running; the message was not delivered"
                })
                .to_string(),
                metadata: None,
            });
        }
        let child_thread_id = task.child_thread_id.clone().ok_or_else(|| {
            AgentError::ToolExecution(format!("task {task_id} has no child thread bound"))
        })?;

        let steering = ConversationMessage {
            role: "user".to_owned(),
            content: ConversationMessageContent::Text(format!(
                "[steering from parent agent]\n{message}"
            )),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        };
        let content = match self.control.queue_input(&child_thread_id, steering).await? {
            SendOutcome::Queued { position } => serde_json::json!({
                "queued": true,
                "position": position,
            }),
            SendOutcome::NeedsResume => serde_json::json!({
                "queued": false,
                "note": "subagent is not running; the message was not delivered"
            }),
        };
        Ok(ToolOutput { content: content.to_string(), metadata: None })
    }
}

/// `subagent_stop`: cancel a running background subagent delegation.
pub struct SubagentStopTool {
    registry: Arc<BackgroundTaskRegistry>,
}

impl SubagentStopTool {
    pub fn new(registry: Arc<BackgroundTaskRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl ToolHandler for SubagentStopTool {
    fn name(&self) -> &str {
        "subagent_stop"
    }

    fn description(&self) -> &str {
        "Stop a running background subagent delegation: interrupts the child \
         agent thread and reports the resulting status. A stopped subagent \
         does NOT deliver a completion message."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "Task id returned by delegate_subagent."
                }
            },
            "required": ["task_id"]
        })
    }

    /// Touches only the task's own child thread — no shared workspace state.
    fn is_concurrency_safe(&self, _arguments: &Value) -> bool {
        true
    }

    fn describe_operation(&self, arguments: &Value) -> Option<slab_agent::OperationDescriptor> {
        let task_id = arguments.get("task_id").and_then(Value::as_str)?;
        Some(slab_agent::OperationDescriptor::read_only(format!("subagent_stop: {task_id}")))
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let task_id = arguments
            .get("task_id")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::ToolExecution("missing 'task_id' argument".into()))?;
        // Kind-check first so shell ids get a precise error, then stop.
        subagent_snapshot(&self.registry, task_id)?;
        let task = self.registry.stop(task_id)?;
        Ok(ToolOutput {
            content: serde_json::json!({ "stopped": subagent_snapshot_json(&task) }).to_string(),
            metadata: None,
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn summarize_task_keeps_one_short_line() {
        let summarize = crate::subagent::summarize_task_for_registry;
        assert_eq!(summarize("hello"), "hello");
        let long = "x".repeat(120);
        let summarized = summarize(&long);
        assert_eq!(summarized.chars().count(), 81); // 80 chars + ellipsis
        let multi = "first line\nsecond line";
        assert_eq!(summarize(multi), "first line");
    }
}
