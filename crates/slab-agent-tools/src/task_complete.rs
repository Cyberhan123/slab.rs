//! Deterministic default-deny task completion tool.
//!
//! The agent must call `task.complete` to finish a task. The tool denies
//! completion unless the submitted plan is non-empty and every item is marked
//! `completed`, keeping the completion decision in deterministic hands instead
//! of trusting the model's self-assessment (Anthropic anti-pattern: the same
//! LLM confidently confirms its own mistakes).
//!
//! On success the tool returns a structured marker in `ToolOutput::metadata`
//! that the turn loop (`crates/slab-agent`) recognizes to emit the final answer
//! (双轨 2 alongside the existing `tool_calls.is_empty()` Final). On denial it
//! returns `AgentError::ToolExecution`, which the turn loop records as a failed
//! tool result and feeds back to the LLM so it can keep working.
//!
//! Metadata contract (consumed by `slab-agent::turn_tool_call`):
//! ```json
//! { "task_complete": { "summary": "...", "artifact_refs": [{ "path": "...", "kind": "file" }] } }
//! ```

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput};

/// Tool name recognized by the agent turn loop as the structured-completion
/// signal. Mirrored as a literal in `crates/slab-agent::turn_tool_call` because
/// `slab-agent` cannot depend on this crate (dependency direction is reversed).
pub const TASK_COMPLETE_TOOL_NAME: &str = "task.complete";

/// Metadata key placed in [`ToolOutput::metadata`] on a successful completion.
pub const TASK_COMPLETE_METADATA_KEY: &str = "task_complete";

#[derive(Default)]
pub struct TaskCompleteTool;

impl TaskCompleteTool {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Deserialize)]
struct TaskCompleteArgs {
    summary: String,
    #[serde(default)]
    plan: Vec<TaskPlanItemInput>,
    #[serde(default)]
    artifact_refs: Vec<ArtifactRefInput>,
    #[serde(default)]
    followup_actions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TaskPlanItemInput {
    step: String,
    status: TaskPlanStatus,
    /// Optional reference to a deterministic verify result (e.g. from `verify`).
    #[serde(default)]
    result_ref: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TaskPlanStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
}

impl TaskPlanStatus {
    fn is_completed(&self) -> bool {
        matches!(self, Self::Completed)
    }
}

#[derive(Debug, Deserialize)]
struct ArtifactRefInput {
    path: String,
    #[serde(default)]
    kind: Option<String>,
}

#[async_trait]
impl ToolHandler for TaskCompleteTool {
    fn name(&self) -> &str {
        TASK_COMPLETE_TOOL_NAME
    }

    fn description(&self) -> &str {
        "Signal that the task is complete. Denied unless every plan item is completed; on success the run ends with the summary as the final answer."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "summary": {
                    "type": "string",
                    "description": "Concise summary of what was accomplished; becomes the final answer text."
                },
                "plan": {
                    "type": "array",
                    "minItems": 1,
                    "description": "The final plan snapshot. Every item must be completed or completion is denied.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "step": { "type": "string" },
                            "status": { "enum": ["pending", "in_progress", "completed", "blocked"] },
                            "result_ref": { "type": "string", "description": "Optional reference to a verify result." }
                        },
                        "required": ["step", "status"]
                    }
                },
                "artifact_refs": {
                    "type": "array",
                    "description": "Workspace-relative artifacts produced by the task.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" },
                            "kind": { "enum": ["file", "diff", "image"] }
                        },
                        "required": ["path"]
                    }
                },
                "followup_actions": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional suggested follow-up actions surfaced to the user."
                }
            },
            "required": ["summary", "plan"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let args: TaskCompleteArgs =
            serde_json::from_value(arguments.clone()).map_err(|error| {
                AgentError::ToolExecution(format!("invalid task.complete args: {error}"))
            })?;

        let summary = args.summary.trim();
        if summary.is_empty() {
            return Err(AgentError::ToolExecution(
                "task.complete requires a non-empty summary".to_owned(),
            ));
        }

        if args.plan.is_empty() {
            return Err(AgentError::ToolExecution(
                "task.complete denied: plan must contain at least one item".to_owned(),
            ));
        }

        let incomplete_steps: Vec<&str> = args
            .plan
            .iter()
            .filter(|item| !item.status.is_completed())
            .map(|item| item.step.as_str())
            .collect();
        if !incomplete_steps.is_empty() {
            return Err(AgentError::ToolExecution(format!(
                "task.complete denied: {} plan item(s) are not completed ({}); finish or update them before completing",
                incomplete_steps.len(),
                incomplete_steps.join(", ")
            )));
        }

        let plan_items = args.plan.len();
        let plan_verified = args
            .plan
            .iter()
            .filter(|item| {
                item.result_ref.as_deref().map(str::trim).is_some_and(|value| !value.is_empty())
            })
            .count();
        let followup_actions = args
            .followup_actions
            .iter()
            .map(|action| action.trim())
            .filter(|action| !action.is_empty())
            .collect::<Vec<_>>();

        let artifact_refs: Vec<Value> = args
            .artifact_refs
            .iter()
            .filter_map(|artifact| normalize_artifact_ref(&artifact.path, artifact.kind.as_deref()))
            .collect();

        let metadata = json!({
            TASK_COMPLETE_METADATA_KEY: {
                "summary": summary,
                "artifact_refs": artifact_refs,
                "plan": { "items": plan_items, "verified": plan_verified },
                "followup_actions": followup_actions,
            }
        });
        let content = format!("task complete: {summary}");

        Ok(ToolOutput { content, metadata: Some(metadata) })
    }
}

fn normalize_artifact_ref(path: &str, kind: Option<&str>) -> Option<Value> {
    let normalized = normalize_workspace_relative_path(path)?;
    let kind = match kind.map(str::to_ascii_lowercase).as_deref() {
        Some("diff") => "diff",
        Some("image") => "image",
        _ => "file",
    };
    Some(json!({ "path": normalized, "kind": kind }))
}

fn normalize_workspace_relative_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() || is_absolute_or_drive_path(trimmed) {
        return None;
    }

    let normalized = trimmed.replace('\\', "/");
    let parts =
        normalized.split('/').filter(|part| !part.is_empty() && *part != ".").collect::<Vec<_>>();
    if parts.is_empty() || parts.contains(&"..") {
        return None;
    }

    Some(parts.join("/"))
}

fn is_absolute_or_drive_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    path.starts_with('/')
        || path.starts_with('\\')
        || (bytes.first().is_some_and(u8::is_ascii_alphabetic) && bytes.get(1) == Some(&b':'))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use slab_agent::{AgentError, ToolContext, ToolHandler};

    use super::*;

    fn ctx() -> ToolContext {
        ToolContext::for_thread("thread").build()
    }

    fn completed_plan() -> Value {
        json!({
            "summary": "  shipped the fix  ",
            "plan": [
                { "step": "investigate", "status": "completed", "result_ref": "verify:lint:passed" },
                { "step": "implement", "status": "completed" }
            ],
            "artifact_refs": [
                { "path": "src\\main.rs", "kind": "file" },
                { "path": "src/lib.rs", "kind": "diff" }
            ]
        })
    }

    #[tokio::test]
    async fn task_complete_succeeds_when_plan_fully_completed() {
        let tool = TaskCompleteTool::new();
        let output = tool.execute(&ctx(), &completed_plan()).await.expect("plan is complete");

        let metadata = output.metadata.expect("metadata marker present");
        assert_eq!(metadata[TASK_COMPLETE_METADATA_KEY]["summary"], "shipped the fix");
        assert_eq!(output.content, "task complete: shipped the fix");

        let refs = metadata[TASK_COMPLETE_METADATA_KEY]["artifact_refs"].as_array().unwrap();
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0]["path"], "src/main.rs");
        assert_eq!(refs[0]["kind"], "file");
        assert_eq!(refs[1]["kind"], "diff");
    }

    #[tokio::test]
    async fn task_complete_denied_when_plan_has_incomplete_items() {
        let tool = TaskCompleteTool::new();
        let args = json!({
            "summary": "done",
            "plan": [
                { "step": "investigate", "status": "completed" },
                { "step": "implement", "status": "in_progress" }
            ]
        });
        let error = tool.execute(&ctx(), &args).await.expect_err("incomplete plan denied");

        assert!(matches!(error, AgentError::ToolExecution(_)));
        assert!(error.to_string().contains("1 plan item(s) are not completed"));
    }

    #[tokio::test]
    async fn task_complete_denied_when_plan_empty() {
        let tool = TaskCompleteTool::new();
        let error = tool
            .execute(&ctx(), &json!({ "summary": "done", "plan": [] }))
            .await
            .expect_err("empty plan denied");

        assert!(error.to_string().contains("at least one item"));
    }

    #[tokio::test]
    async fn task_complete_denied_when_summary_blank() {
        let tool = TaskCompleteTool::new();
        let error = tool
            .execute(
                &ctx(),
                &json!({ "summary": "   ", "plan": [{ "step": "x", "status": "completed" }] }),
            )
            .await
            .expect_err("blank summary denied");

        assert!(error.to_string().contains("non-empty summary"));
    }

    #[tokio::test]
    async fn task_complete_drops_unsafe_artifact_refs() {
        let tool = TaskCompleteTool::new();
        let args = json!({
            "summary": "done",
            "plan": [{ "step": "x", "status": "completed" }],
            "artifact_refs": [
                { "path": "../outside.rs" },
                { "path": "/etc/passwd" },
                { "path": "C:/Users/me/.ssh/id_rsa" },
                { "path": "src/ok.rs", "kind": "image" }
            ]
        });
        let output = tool.execute(&ctx(), &args).await.expect("plan complete");

        let metadata = output.metadata.unwrap();
        let refs = metadata[TASK_COMPLETE_METADATA_KEY]["artifact_refs"].as_array().unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0]["path"], "src/ok.rs");
        assert_eq!(refs[0]["kind"], "image");
    }

    #[test]
    fn task_complete_schema_requires_summary_and_plan() {
        let schema = TaskCompleteTool::new().parameters_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "summary"));
        assert!(required.iter().any(|v| v == "plan"));
    }
}
