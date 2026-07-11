//! Shell command execution tool backed by `slab-shell-command`.
//!
//! Permission decisions are owned by `slab-exec-policy`; this tool only
//! describes its operation (`describe_operation`) and executes when the kernel
//! has authorized it.

use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput};
use slab_sandboxing::SandboxDriver;
pub use slab_shell_command::ShellPolicy;
use slab_shell_command::{ShellCommand, ShellExecutor};

pub struct ShellTool {
    executor: ShellExecutor,
}

impl ShellTool {
    pub fn new(
        workspace_root: Option<PathBuf>,
        sandbox_driver: Option<Arc<dyn SandboxDriver>>,
    ) -> Self {
        Self { executor: ShellExecutor::new(workspace_root, sandbox_driver) }
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new(None, None)
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

    fn describe_operation(&self, arguments: &Value) -> Option<slab_agent::OperationDescriptor> {
        let command = arguments.get("command").and_then(Value::as_str)?.to_string();
        Some(slab_agent::OperationDescriptor::shell(command))
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

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use serde_json::{Value, json};
    use slab_agent::{ToolContext, ToolHandler};
    use slab_sandboxing::{SandboxError, SandboxedCommand, SandboxedOutput};

    use super::*;

    fn ctx() -> ToolContext {
        ToolContext::for_thread("thread").build()
    }

    #[derive(Clone)]
    struct RecordingDriver {
        seen: Arc<Mutex<Option<SandboxedCommand>>>,
        output: SandboxedOutput,
    }

    #[async_trait]
    impl SandboxDriver for RecordingDriver {
        fn name(&self) -> &str {
            "recording"
        }

        async fn run(&self, cmd: SandboxedCommand) -> Result<SandboxedOutput, SandboxError> {
            *self.seen.lock().unwrap() = Some(cmd);
            Ok(self.output.clone())
        }
    }

    #[tokio::test]
    async fn shell_tool_maps_sandbox_output_to_json_and_filters_env_values() {
        let seen = Arc::new(Mutex::new(None));
        let tool = ShellTool::new(
            Some(PathBuf::from("workspace")),
            Some(Arc::new(RecordingDriver {
                seen: Arc::clone(&seen),
                output: SandboxedOutput {
                    stdout: b"ok".to_vec(),
                    stderr: b"warn".to_vec(),
                    exit_code: 7,
                    timed_out: true,
                },
            })),
        );

        let output = tool
            .execute(
                &ctx(),
                &json!({
                    "command": "echo ok",
                    "timeout_secs": 5,
                    "env": {
                        "TEXT": "value",
                        "IGNORED": false
                    }
                }),
            )
            .await
            .expect("shell output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert_eq!(value["stdout"], "ok");
        assert_eq!(value["stderr"], "warn");
        assert_eq!(value["exit_code"], 7);
        assert_eq!(value["timed_out"], true);

        let command = seen.lock().unwrap().clone().expect("driver command");
        assert_eq!(command.cwd.as_deref(), Some(PathBuf::from("workspace").as_path()));
        assert_eq!(command.timeout.map(|timeout| timeout.as_secs()), Some(5));
        assert_eq!(command.env.get("TEXT").map(String::as_str), Some("value"));
        assert!(!command.env.contains_key("IGNORED"));
    }

    #[tokio::test]
    async fn shell_tool_rejects_missing_command_and_dangerous_command() {
        let tool = ShellTool::new(None, None);

        let missing = tool.execute(&ctx(), &json!({})).await.expect_err("missing command");
        assert_eq!(missing.to_string(), "tool execution error: missing 'command' argument");

        let dangerous = tool
            .execute(&ctx(), &json!({"command": "rm -rf /"}))
            .await
            .expect_err("dangerous command");
        assert!(dangerous.to_string().contains("command blocked"));
    }

    #[test]
    fn shell_tool_describes_operation_as_shell_command() {
        let tool = ShellTool::new(None, None);
        let descriptor =
            tool.describe_operation(&json!({"command": "cargo check"})).expect("descriptor");
        assert_eq!(descriptor.category, slab_agent::OperationCategory::Shell);
        assert_eq!(descriptor.subject, "cargo check");
    }

    #[test]
    fn shell_tool_schema_marks_command_required() {
        let schema = ShellTool::default().parameters_schema();

        assert_eq!(schema["properties"]["command"]["type"], "string");
        assert_eq!(schema["properties"]["env"]["additionalProperties"]["type"], "string");
        assert_eq!(schema["required"], json!(["command"]));
    }
}
