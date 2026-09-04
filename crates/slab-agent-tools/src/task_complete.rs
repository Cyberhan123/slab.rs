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
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use slab_agent::{AgentError, ToolContext, ToolOutput, TypedTool, typed_input_schema};

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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskCompleteArgs {
    /// Concise summary of what was accomplished; becomes the final answer text.
    summary: String,
    /// The final plan snapshot. Every item must be completed or completion is denied.
    #[schemars(length(min = 1))]
    plan: Vec<TaskPlanItemInput>,
    /// Workspace-relative artifacts produced by the task.
    #[serde(default)]
    artifact_refs: Vec<ArtifactRefInput>,
    /// Optional suggested follow-up actions surfaced to the user.
    #[serde(default)]
    followup_actions: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(inline)]
struct TaskPlanItemInput {
    step: String,
    status: TaskPlanStatus,
    /// Optional reference to a verify result.
    result_ref: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(inline)]
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

// Schema-only hint for `ArtifactRefInput::kind`: the runtime keeps `kind` as
// a free-form string so unknown kinds are surfaced, not rejected. (Deliberate
// plain comment — a doc comment would leak into the generated schema as a
// description.)
#[derive(JsonSchema)]
#[schemars(inline)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // schema-only mirror; variants are never constructed
enum ArtifactKindSchema {
    File,
    Diff,
    Image,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(inline)]
struct ArtifactRefInput {
    path: String,
    #[schemars(with = "Option<ArtifactKindSchema>")]
    kind: Option<String>,
}

#[async_trait]
impl TypedTool for TaskCompleteTool {
    type Input = TaskCompleteArgs;
    fn name(&self) -> &str {
        TASK_COMPLETE_TOOL_NAME
    }

    fn description(&self) -> &str {
        "Signal that the task is complete. Denied unless every plan item is completed; on success the run ends with the summary as the final answer."
    }

    fn parameters_schema(&self) -> Value {
        typed_input_schema::<TaskCompleteArgs>()
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        args: TaskCompleteArgs,
    ) -> Result<ToolOutput, AgentError> {
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

        // Replay guard: a completed run retires its durable plan (at the Final
        // transition and at run teardown), so an active plan in the store is
        // the deterministic proof that THIS task was planned through
        // `update_plan`. A completion arriving with no active plan is a replay
        // of a previous iteration's completion (or a completion that skipped
        // planning entirely) — deny with the recovery path instead of
        // finalizing on stale context.
        if ctx.plan_store.current_plan(&ctx.thread_id).await.is_none() {
            return Err(AgentError::ToolExecution(
                "task.complete denied: no active plan on this thread (a completed run retires its plan); call update_plan with the plan for the current task, then complete".to_owned(),
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
    use std::sync::{Arc, Mutex};

    use serde_json::{Value, json};
    use slab_agent::{
        AgentError, Plan, PlanCounts, PlanItem, PlanStatus, PlanStorePort, ToolContext, ToolHandler,
    };

    use super::*;

    /// Plan store double pre-seeded with an active plan — the deterministic
    /// proof `task.complete` requires (mirrors the app-core in-memory impl).
    #[derive(Default)]
    struct SeededPlanStore {
        plan: Mutex<Option<Plan>>,
    }

    #[async_trait::async_trait]
    impl PlanStorePort for SeededPlanStore {
        async fn replace_plan(&self, _thread_id: &str, plan: Plan) -> Result<(), AgentError> {
            *self.plan.lock().unwrap() = Some(plan);
            Ok(())
        }
        async fn current_plan(&self, _thread_id: &str) -> Option<Plan> {
            self.plan.lock().unwrap().clone()
        }
        async fn clear(&self, _thread_id: &str) {
            *self.plan.lock().unwrap() = None;
        }
    }

    fn active_plan() -> Plan {
        Plan {
            plan_id: "plan-1".to_owned(),
            summary: Some("active task".to_owned()),
            items: vec![PlanItem {
                step: "work".to_owned(),
                status: PlanStatus::Completed,
                depends_on: None,
                result_ref: None,
            }],
            counts: PlanCounts { pending: 0, in_progress: 0, completed: 1, blocked: 0 },
            current_step: None,
        }
    }

    fn ctx() -> ToolContext {
        let store = SeededPlanStore { plan: Mutex::new(Some(active_plan())) };
        ToolContext::for_thread("thread").plan_store(Arc::new(store)).build()
    }

    fn empty_store_ctx() -> ToolContext {
        ToolContext::for_thread("thread").plan_store(Arc::new(SeededPlanStore::default())).build()
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
        let output =
            ToolHandler::execute(&tool, &ctx(), &completed_plan()).await.expect("plan is complete");

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
        let error =
            ToolHandler::execute(&tool, &ctx(), &args).await.expect_err("incomplete plan denied");

        assert!(matches!(error, AgentError::ToolExecution(_)));
        assert!(error.to_string().contains("1 plan item(s) are not completed"));
    }

    #[tokio::test]
    async fn task_complete_denied_when_plan_empty() {
        let tool = TaskCompleteTool::new();
        let error = ToolHandler::execute(&tool, &ctx(), &json!({ "summary": "done", "plan": [] }))
            .await
            .expect_err("empty plan denied");

        assert!(error.to_string().contains("at least one item"));
    }

    #[tokio::test]
    async fn task_complete_denied_when_summary_blank() {
        let tool = TaskCompleteTool::new();
        let error = ToolHandler::execute(
            &tool,
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
        let output = ToolHandler::execute(&tool, &ctx(), &args).await.expect("plan complete");

        let metadata = output.metadata.unwrap();
        let refs = metadata[TASK_COMPLETE_METADATA_KEY]["artifact_refs"].as_array().unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0]["path"], "src/ok.rs");
        assert_eq!(refs[0]["kind"], "image");
    }

    #[test]
    fn task_complete_schema_requires_summary_and_plan() {
        let schema = ToolHandler::parameters_schema(&TaskCompleteTool::new());
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "summary"));
        assert!(required.iter().any(|v| v == "plan"));
        // The artifact kind stays optional with its enum hint intact (an
        // optional enum advertises null alongside the values).
        let artifact_props = &schema["properties"]["artifact_refs"]["items"]["properties"];
        assert!(!required.iter().any(|v| v == "kind"));
        assert_eq!(artifact_props["kind"]["enum"], json!(["file", "diff", "image", null]));
    }

    #[tokio::test]
    async fn task_complete_denied_when_no_active_plan_in_store() {
        // A completed run retires its plan; replaying the same completion (a
        // resumed run finalizing on stale context) must be denied with the
        // recovery path instead of silently finalizing.
        let tool = TaskCompleteTool::new();
        let error = ToolHandler::execute(&tool, &empty_store_ctx(), &completed_plan())
            .await
            .expect_err("replay without an active plan denied");

        assert!(matches!(error, AgentError::ToolExecution(_)));
        let message = error.to_string();
        assert!(message.contains("no active plan"), "{message}");
        assert!(message.contains("update_plan"), "{message}");
    }
}
