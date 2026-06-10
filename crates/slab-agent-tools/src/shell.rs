//! Shell command execution tool backed by `slab-shell-command`.

use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{AgentError, ToolApprovalRequest, ToolContext, ToolHandler, ToolOutput};
use slab_sandboxing::SandboxDriver;
pub use slab_shell_command::ShellPolicy;
use slab_shell_command::{ShellCommand, ShellExecutor, ShellRuleSet};

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

    pub fn new_with_rules(
        policy: ShellPolicy,
        workspace_root: Option<PathBuf>,
        sandbox_driver: Option<Arc<dyn SandboxDriver>>,
        rules: ShellRuleSet,
    ) -> Self {
        Self {
            executor: ShellExecutor::new(policy, workspace_root, sandbox_driver).with_rules(rules),
        }
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
        let command = arguments.get("command").and_then(Value::as_str)?.to_string();
        if !self.executor.approval_required_for_command(&command) {
            return None;
        }
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
    use slab_shell_command::{ShellRule, ShellRuleAction, ShellRuleMatcher};

    use super::*;

    fn ctx() -> ToolContext {
        ToolContext { thread_id: "thread".into(), turn_index: 0, depth: 0 }
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
            ShellPolicy::Allow,
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
    async fn shell_tool_rejects_missing_command_and_policy_blocks() {
        let blocked = ShellTool::new(ShellPolicy::Block, None, None);

        let missing = blocked.execute(&ctx(), &json!({})).await.expect_err("missing command");
        assert_eq!(missing.to_string(), "tool execution error: missing 'command' argument");

        let blocked_error = blocked
            .execute(&ctx(), &json!({"command": "echo blocked"}))
            .await
            .expect_err("blocked command");
        assert!(blocked_error.to_string().contains("blocked by policy"));
    }

    #[test]
    fn shell_tool_approval_respects_policy_and_rules() {
        let review = ShellTool::new(ShellPolicy::RequireApproval, None, None);
        assert_eq!(
            review
                .approval_request(&json!({"command": "echo review"}))
                .map(|request| request.command),
            Some("echo review".to_string())
        );
        assert!(review.approval_request(&json!({"command": false})).is_none());

        let rules = ShellRuleSet::from_rules(vec![ShellRule::new(
            ShellRuleAction::Allow,
            ShellRuleMatcher::Prefix,
            "cargo check",
        )]);
        let auto = ShellTool::new_with_rules(ShellPolicy::RequireApproval, None, None, rules);

        assert!(auto.approval_request(&json!({"command": "cargo check -p slab-agent"})).is_none());
        assert!(auto.approval_request(&json!({"command": "cargo test -p slab-agent"})).is_some());
    }

    #[test]
    fn shell_tool_schema_marks_command_required() {
        let schema = ShellTool::default().parameters_schema();

        assert_eq!(schema["properties"]["command"]["type"], "string");
        assert_eq!(schema["properties"]["env"]["additionalProperties"]["type"], "string");
        assert_eq!(schema["required"], json!(["command"]));
    }
}
