//! Shell command execution tool backed by `slab-shell-command`.
//!
//! Permission decisions are owned by `slab-exec-policy`; this tool only
//! describes its operation (`describe_operation`) and executes when the kernel
//! has authorized it.

use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::protocol::TurnItem;
use slab_agent::{
    AgentError, ToolCallRender, ToolContext, ToolHandler, ToolOutput, ToolOutputObserver,
    ToolOutputStream,
};
use slab_sandboxing::{OutputSink, OutputStream, SandboxDriver};
pub use slab_shell_command::ShellPolicy;
use slab_shell_command::{ShellCommand, ShellExecutor, ShellLauncher};

/// Adapts the agent-side [`ToolOutputObserver`] to the sandbox's [`OutputSink`],
/// mapping stream tags 1:1.
struct ToolObserverSink(Arc<dyn ToolOutputObserver>);

impl OutputSink for ToolObserverSink {
    fn on_output(&self, stream: OutputStream, delta: &str) {
        let mapped = match stream {
            OutputStream::Stdout => ToolOutputStream::Stdout,
            OutputStream::Stderr => ToolOutputStream::Stderr,
        };
        self.0.on_output(mapped, delta);
    }
}

pub struct ShellTool {
    executor: ShellExecutor,
}

impl ShellTool {
    pub fn new(
        workspace_root: Option<PathBuf>,
        sandbox_driver: Option<Arc<dyn SandboxDriver>>,
        launcher: ShellLauncher,
        bash_path: Option<PathBuf>,
    ) -> Self {
        Self { executor: ShellExecutor::new(workspace_root, sandbox_driver, launcher, bash_path) }
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new(None, None, ShellLauncher::default(), None)
    }
}

#[async_trait]
impl ToolHandler for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return stdout, stderr, exit_code, and timeout status. \
         On timeout the process tree is killed and exit_code is 124."
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

    fn category(&self) -> slab_agent::OperationCategory {
        slab_agent::OperationCategory::Shell
    }

    fn render_turn_item(&self, render: &ToolCallRender<'_>) -> TurnItem {
        TurnItem::CommandExecution {
            id: render.call.id.clone(),
            command: render.args.get("command").and_then(Value::as_str).unwrap_or("").to_owned(),
            cwd: render.workspace_root.unwrap_or("").to_owned(),
            process_id: None,
            status: render.status.to_owned(),
            aggregated_output: render.output.map(str::to_owned),
            exit_code: render.exit_code,
            duration_ms: render.duration_ms,
        }
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let command = arguments
            .get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::ToolExecution("missing 'command' argument".into()))?
            .to_string();
        // Models sometimes pretty-print tool-call JSON with the command value
        // starting (or ending) on its own line, e.g. {"command": "\nls -la"}.
        // `bash -lc $'\nls'` then treats the command as starting on line 2 and
        // reports `line 2: syntax error` for perfectly valid single-line
        // commands — trim both ends before execution. (Fixing this at the tool
        // layer covers every producer: streaming increments, non-streaming
        // calls, and subagents.)
        let command = command.trim().to_string();
        if command.is_empty() {
            return Err(AgentError::ToolExecution("'command' is empty".into()));
        }
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

        // When the agent attached a live-output observer, stream stdout/stderr
        // chunks through it while the command runs (display-only).
        let sink = ctx.output.as_ref().map(|observer| {
            Arc::new(ToolObserverSink(Arc::clone(observer))) as Arc<dyn OutputSink>
        });

        let output = self
            .executor
            .execute_with_sink(ShellCommand { command, timeout_secs, env }, sink)
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

    #[test]
    fn shell_tool_renders_command_execution() {
        let tool = ShellTool::default();
        let call = slab_agent::port::ParsedToolCall {
            id: "c1".into(),
            name: "shell".into(),
            arguments: r#"{"command":"ls"}"#.into(),
        };
        let args = json!({"command": "ls -la"});
        let render = ToolCallRender {
            call: &call,
            args: &args,
            status: "running",
            output: None,
            workspace_root: Some("/ws"),
            exit_code: None,
            duration_ms: None,
        };
        match tool.render_turn_item(&render) {
            TurnItem::CommandExecution { command, cwd, status, aggregated_output, .. } => {
                assert_eq!(command, "ls -la");
                assert_eq!(cwd, "/ws");
                assert_eq!(status, "running");
                assert!(aggregated_output.is_none());
            }
            other => panic!("unexpected item: {other:?}"),
        }
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
            ShellLauncher::PowerShell,
            None,
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
        let tool = ShellTool::new(None, None, ShellLauncher::default(), None);

        let missing = tool.execute(&ctx(), &json!({})).await.expect_err("missing command");
        assert_eq!(missing.to_string(), "tool execution error: missing 'command' argument");

        let dangerous = tool
            .execute(&ctx(), &json!({"command": "rm -rf /"}))
            .await
            .expect_err("dangerous command");
        assert!(dangerous.to_string().contains("command blocked"));
    }

    /// P8 regression: leading/trailing newlines around the command (from
    /// pretty-printed tool-call JSON) must be trimmed so `bash -lc` does not
    /// see the command "starting on line 2".
    #[tokio::test]
    async fn shell_tool_trims_leading_and_trailing_whitespace() {
        let seen = Arc::new(Mutex::new(None));
        let tool = ShellTool::new(
            None,
            Some(Arc::new(RecordingDriver {
                seen: Arc::clone(&seen),
                output: SandboxedOutput {
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                    exit_code: 0,
                    timed_out: false,
                },
            })),
            ShellLauncher::PowerShell,
            None,
        );

        tool.execute(&ctx(), &json!({"command": "\n  echo trimmed  \n"})).await.expect("execute");

        let command = seen.lock().unwrap().clone().expect("driver command");
        assert_eq!(command.argv.last().map(String::as_str), Some("echo trimmed"));
    }

    #[tokio::test]
    async fn shell_tool_rejects_blank_command() {
        let tool = ShellTool::new(None, None, ShellLauncher::default(), None);

        let blank = tool.execute(&ctx(), &json!({"command": "  \n "})).await.expect_err("blank");
        assert!(blank.to_string().contains("'command' is empty"));
    }

    #[test]
    fn shell_tool_describes_operation_as_shell_command() {
        let tool = ShellTool::new(None, None, ShellLauncher::default(), None);
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

    /// Faithful mirror of `turn_tool_call::handle_tool_call`: a run-local
    /// `ToolContext` carries a channel-backed observer; `ShellTool::execute`
    /// runs concurrently with a drain via `tokio::join!`. The sender drops when
    /// the run block ends (execute completes), so the drain must terminate.
    #[tokio::test]
    async fn shell_tool_execute_in_join_drain_completes() {
        use slab_agent::{ToolOutputObserver, ToolOutputStream};
        use std::time::Duration;

        struct ChannelObserver {
            sender: tokio::sync::mpsc::UnboundedSender<String>,
        }
        impl ToolOutputObserver for ChannelObserver {
            fn on_output(&self, _stream: ToolOutputStream, delta: &str) {
                let _ = self.sender.send(delta.to_string());
            }
        }

        let tool = ShellTool::new(Some(PathBuf::from(".")), None, ShellLauncher::Auto, None);
        let args = json!({ "command": "echo join-drain-marker" });
        let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        let run = async {
            let mut ctx = ToolContext::for_thread("thread").build();
            ctx.output =
                Some(Arc::new(ChannelObserver { sender: delta_tx }) as Arc<dyn ToolOutputObserver>);
            tool.execute(&ctx, &args).await
        };
        let drain = async {
            while let Some(delta) = delta_rx.recv().await {
                let _ = delta;
            }
        };

        let (result, ()) =
            tokio::time::timeout(Duration::from_secs(20), async { tokio::join!(run, drain) })
                .await
                .expect("join!(execute, drain) hung past 20s");
        let output = result.expect("execute");
        let value: Value = serde_json::from_str(&output.content).expect("json");
        assert_eq!(value["exit_code"], 0, "stderr: {}", value["stderr"]);
    }

    /// Full production combination: `ShellTool` + the real platform sandbox
    /// driver + a channel-backed observer drained concurrently via `join!`.
    /// Reproduces the post-approval execute path end-to-end.
    #[tokio::test]
    async fn shell_tool_with_platform_driver_in_join_drain_completes() {
        use slab_agent::{ToolOutputObserver, ToolOutputStream};
        use slab_sandboxing::{SandboxEnvironment, SandboxPolicy, create_platform_driver};
        use std::time::Duration;

        struct ChannelObserver {
            sender: tokio::sync::mpsc::UnboundedSender<String>,
        }
        impl ToolOutputObserver for ChannelObserver {
            fn on_output(&self, _stream: ToolOutputStream, delta: &str) {
                let _ = self.sender.send(delta.to_string());
            }
        }

        let workspace = PathBuf::from(".");
        let env = SandboxEnvironment::new(Some(workspace.clone()), SandboxPolicy::WorkspaceWrite);
        let driver = create_platform_driver(env).expect("platform driver");
        if !driver.setup_status().available {
            eprintln!("skipping: platform driver unavailable");
            return;
        }

        let tool = ShellTool::new(Some(workspace), Some(driver), ShellLauncher::Auto, None);
        let args = json!({ "command": "echo prod-drain-marker" });
        let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        let run = async {
            let mut ctx = ToolContext::for_thread("thread").build();
            ctx.output =
                Some(Arc::new(ChannelObserver { sender: delta_tx }) as Arc<dyn ToolOutputObserver>);
            tool.execute(&ctx, &args).await
        };
        let drain = async {
            while let Some(delta) = delta_rx.recv().await {
                let _ = delta;
            }
        };

        let (result, ()) =
            tokio::time::timeout(Duration::from_secs(20), async { tokio::join!(run, drain) })
                .await
                .expect("join!(execute, drain) with platform driver hung past 20s");
        let output = result.expect("execute");
        let value: Value = serde_json::from_str(&output.content).expect("json");
        assert_eq!(value["exit_code"], 0, "stderr: {}", value["stderr"]);
    }
}
