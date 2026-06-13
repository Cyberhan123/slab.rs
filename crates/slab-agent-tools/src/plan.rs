//! Plan update tool for agent-visible todo state.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput};

pub struct PlanUpdateTool;

impl PlanUpdateTool {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Deserialize)]
struct PlanUpdateArgs {
    #[serde(default)]
    summary: Option<String>,
    items: Vec<PlanItemInput>,
}

#[derive(Debug, Deserialize)]
struct PlanItemInput {
    step: String,
    status: PlanStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PlanStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
}

impl PlanStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Blocked => "blocked",
        }
    }
}

#[async_trait]
impl ToolHandler for PlanUpdateTool {
    fn name(&self) -> &str {
        "plan_update"
    }

    fn description(&self) -> &str {
        "Record the current execution plan or todo list so it can be replayed in the agent timeline."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "summary": {
                    "type": "string",
                    "description": "Optional short summary of what this plan is tracking."
                },
                "items": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "properties": {
                            "step": {
                                "type": "string",
                                "description": "A concrete task or checkpoint."
                            },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed", "blocked"]
                            }
                        },
                        "required": ["step", "status"]
                    }
                }
            },
            "required": ["items"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let args: PlanUpdateArgs = serde_json::from_value(arguments.clone()).map_err(|error| {
            AgentError::ToolExecution(format!("invalid plan_update args: {error}"))
        })?;
        let plan = normalize_plan(args)?;

        Ok(ToolOutput {
            content: serde_json::to_string(&plan)
                .map_err(|error| AgentError::ToolExecution(error.to_string()))?,
            metadata: None,
        })
    }
}

fn normalize_plan(args: PlanUpdateArgs) -> Result<Value, AgentError> {
    if args.items.is_empty() {
        return Err(AgentError::ToolExecution("plan_update requires at least one item".to_owned()));
    }

    let mut pending = 0usize;
    let mut in_progress = 0usize;
    let mut completed = 0usize;
    let mut blocked = 0usize;
    let mut current_step = None;
    let mut items = Vec::with_capacity(args.items.len());

    for item in args.items {
        let step = item.step.trim();
        if step.is_empty() {
            return Err(AgentError::ToolExecution(
                "plan_update item step must not be blank".to_owned(),
            ));
        }

        match &item.status {
            PlanStatus::Pending => pending += 1,
            PlanStatus::InProgress => {
                in_progress += 1;
                current_step = Some(step.to_owned());
            }
            PlanStatus::Completed => completed += 1,
            PlanStatus::Blocked => blocked += 1,
        }

        items.push(serde_json::json!({
            "step": step,
            "status": item.status.as_str(),
        }));
    }

    if in_progress > 1 {
        return Err(AgentError::ToolExecution(
            "plan_update accepts at most one in_progress item".to_owned(),
        ));
    }

    Ok(serde_json::json!({
        "summary": args.summary.as_deref().map(str::trim).filter(|value| !value.is_empty()),
        "items": items,
        "counts": {
            "pending": pending,
            "in_progress": in_progress,
            "completed": completed,
            "blocked": blocked
        },
        "current_step": current_step
    }))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use slab_agent::{ToolContext, ToolHandler};

    use super::*;

    fn ctx() -> ToolContext {
        ToolContext { thread_id: "thread".into(), turn_index: 0, depth: 0 }
    }

    #[tokio::test]
    async fn plan_update_returns_normalized_plan() {
        let tool = PlanUpdateTool::new();
        let output = tool
            .execute(
                &ctx(),
                &json!({
                    "summary": "  code change  ",
                    "items": [
                        { "step": " inspect ", "status": "completed" },
                        { "step": "implement", "status": "in_progress" },
                        { "step": "verify", "status": "pending" }
                    ]
                }),
            )
            .await
            .expect("plan output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert_eq!(value["summary"], "code change");
        assert_eq!(value["items"][0], json!({"step": "inspect", "status": "completed"}));
        assert_eq!(value["counts"]["pending"], 1);
        assert_eq!(value["counts"]["in_progress"], 1);
        assert_eq!(value["current_step"], "implement");
    }

    #[tokio::test]
    async fn plan_update_rejects_multiple_current_items() {
        let tool = PlanUpdateTool::new();
        let error = tool
            .execute(
                &ctx(),
                &json!({
                    "items": [
                        { "step": "one", "status": "in_progress" },
                        { "step": "two", "status": "in_progress" }
                    ]
                }),
            )
            .await
            .expect_err("multiple in progress rejected");

        assert!(error.to_string().contains("at most one in_progress"));
    }

    #[tokio::test]
    async fn plan_update_rejects_blank_steps() {
        let tool = PlanUpdateTool::new();
        let error = tool
            .execute(&ctx(), &json!({"items": [{ "step": " ", "status": "pending" }]}))
            .await
            .expect_err("blank step rejected");

        assert!(error.to_string().contains("step must not be blank"));
    }

    #[test]
    fn plan_update_schema_matches_required_arguments() {
        let schema = PlanUpdateTool::new().parameters_schema();

        assert_eq!(schema["properties"]["items"]["type"], "array");
        assert_eq!(
            schema["properties"]["items"]["items"]["properties"]["status"]["enum"],
            json!(["pending", "in_progress", "completed", "blocked"])
        );
        assert_eq!(schema["required"], json!(["items"]));
    }
}
