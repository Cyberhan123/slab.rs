use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use slab_agent::{AgentConfig, AgentControl, AgentError, ToolContext, ToolHandler, ToolOutput};
use slab_types::{ConversationMessage, ConversationMessageContent};

const DEFAULT_SUBAGENT_TURNS: u32 = 8;

pub struct DelegateSubagentTool {
    control: Arc<AgentControl>,
}

impl DelegateSubagentTool {
    pub fn new(control: Arc<AgentControl>) -> Self {
        Self { control }
    }
}

#[derive(Debug, Deserialize)]
struct DelegateSubagentArgs {
    task: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    system_prompt: Option<String>,
    #[serde(default)]
    allowed_tools: Option<Vec<String>>,
    #[serde(default)]
    max_turns: Option<u32>,
}

#[async_trait]
impl ToolHandler for DelegateSubagentTool {
    fn name(&self) -> &str {
        "delegate_subagent"
    }

    fn description(&self) -> &str {
        "Delegate a focused task to an isolated child agent and wait for its result."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "The focused task for the child agent."
                },
                "model": {
                    "type": "string",
                    "description": "Optional model override for the child agent."
                },
                "system_prompt": {
                    "type": "string",
                    "description": "Optional child-agent system prompt."
                },
                "allowed_tools": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional tool allow-list for the child agent."
                },
                "max_turns": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Optional child-agent turn limit."
                }
            },
            "required": ["task"]
        })
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let args: DelegateSubagentArgs =
            serde_json::from_value(arguments.clone()).map_err(|error| {
                AgentError::ToolExecution(format!("invalid subagent args: {error}"))
            })?;
        if args.task.trim().is_empty() {
            return Err(AgentError::ToolExecution("subagent task must not be blank".to_owned()));
        }

        let parent = self
            .control
            .thread_snapshot(&ctx.thread_id)
            .await?
            .ok_or_else(|| AgentError::ThreadNotFound(ctx.thread_id.clone()))?;
        let mut child_config =
            serde_json::from_str::<AgentConfig>(&parent.config_json).map_err(|error| {
                AgentError::ToolExecution(format!("invalid parent agent config: {error}"))
            })?;
        if let Some(model) = args.model.filter(|value| !value.trim().is_empty()) {
            child_config.model = model;
        }
        child_config.system_prompt = Some(args.system_prompt.unwrap_or_else(default_system_prompt));
        if let Some(allowed_tools) = args.allowed_tools {
            child_config.allowed_tools =
                allowed_tools.into_iter().filter(|tool| !tool.trim().is_empty()).collect();
        }
        child_config.max_turns = args.max_turns.unwrap_or(DEFAULT_SUBAGENT_TURNS).max(1);
        child_config.transient = true;

        let messages = vec![ConversationMessage {
            role: "user".to_owned(),
            content: ConversationMessageContent::Text(args.task),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }];
        let child_thread_id =
            self.control.spawn_child_for_parent(&ctx.thread_id, child_config, messages).await?;
        let snapshot = self.control.wait_for_terminal_snapshot(&child_thread_id).await?;

        Ok(ToolOutput {
            content: serde_json::json!({
                "child_thread_id": snapshot.id,
                "status": snapshot.status,
                "completion_text": snapshot.completion_text,
            })
            .to_string(),
            metadata: None,
        })
    }
}

fn default_system_prompt() -> String {
    "You are a focused subagent. Work only on the delegated task, use the allowed tools, and return a concise result for the parent agent.".to_owned()
}
