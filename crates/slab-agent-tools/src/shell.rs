//! Shell command execution tool backed by `slab-shell-command`.

use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{AgentError, ToolApprovalRequest, ToolContext, ToolHandler, ToolOutput};
use slab_sandboxing::SandboxDriver;
pub use slab_shell_command::ShellPolicy;
use slab_shell_command::{ShellCommand, ShellExecutor};

pub struct ShellTool {
    executor: ShellExecutor,
}

impl ShellTool {
    pub fn new(
        policy: ShellPolicy,
        workspace_root: Option<PathBuf>,
        sandbox_driver: Option<Arc<dyn SandboxDriver>>,
    ) -> Self {
        Self { executor: ShellExecutor::new(policy, workspace_root, sandbox_driver) }
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new(ShellPolicy::Allow, None, None)
    }
}

#[async_trait]
impl ToolHandler for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return stdout, stderr, exit_code, and timeout status."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Maximum execution time in seconds.",
                    "default": 30
                },
                "env": {
                    "type": "object",
                    "description": "Environment variables to inject into the command.",
                    "additionalProperties": { "type": "string" },
                    "default": {}
                }
            },
            "required": ["command"]
        })
    }

    fn approval_request(&self, arguments: &Value) -> Option<ToolApprovalRequest> {
        if !self.executor.approval_required() {
            return None;
        }
        let command = arguments.get("command").and_then(Value::as_str)?.to_string();
        Some(ToolApprovalRequest { command })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let command = arguments
            .get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::ToolExecution("missing 'command' argument".into()))?
            .to_string();
        let timeout_secs = arguments.get("timeout_secs").and_then(Value::as_u64).unwrap_or(30);
        let env = arguments
            .get("env")
            .and_then(Value::as_object)
            .map(|env| {
                env.iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let output = self
            .executor
            .execute(ShellCommand { command, timeout_secs, env })
            .await
            .map_err(|e| AgentError::ToolExecution(e.to_string()))?;

        Ok(ToolOutput {
            content: serde_json::to_string(&output)
                .map_err(|e| AgentError::ToolExecution(e.to_string()))?,
            metadata: None,
        })
    }
}
