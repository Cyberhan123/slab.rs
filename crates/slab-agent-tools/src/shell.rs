//! Shell command execution tool backed by `slab-shell-command`.
//!
//! Permission decisions are owned by `slab-exec-policy`; this tool only
//! describes its operation (`describe_operation`) and executes when the kernel
//! has authorized it.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use slab_agent::protocol::TurnItem;
use slab_agent::{
    AgentError, ToolCallRender, ToolContext, ToolOutput, ToolOutputObserver, ToolOutputStream,
    TypedTool,
};
use slab_sandboxing::{OutputSink, OutputStream, SandboxDriver};
pub use slab_shell_command::ShellPolicy;
use slab_shell_command::{DEFAULT_OUTPUT_LIMIT_BYTES, ShellCommand, ShellExecutor, ShellLauncher};
use slab_utils::string::truncate_middle_bytes;

/// Executor capture bound — a memory ceiling for the accumulated stream, well
/// above the context budget so the full (bounded) output can be spilled to
/// disk before truncation.
const SHELL_CAPTURE_LIMIT_BYTES: usize = 512 * 1024;
/// Head fraction of the kept budget when the tool-layer truncation fires.
const CONTEXT_STREAM_HEAD_RATIO: f32 = 0.7;

/// Arguments for the `shell` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ShellArgs {
    /// The shell command to execute.
    command: String,
    /// Maximum execution time in seconds (foreground only; background tasks have no timeout).
    #[serde(default = "default_timeout_secs")]
    timeout_secs: u64,
    /// Run as a detached background task: returns a task_id immediately, output streams to files, poll task_status/task_output.
    #[serde(default)]
    background: bool,
    /// Environment variables to inject into the command.
    #[serde(default, deserialize_with = "deserialize_env")]
    env: HashMap<String, String>,
}

fn default_timeout_secs() -> u64 {
    30
}

/// Keep the historical leniency: non-string env values (and an explicit
/// `null`) are silently dropped instead of failing the whole call.
fn deserialize_env<'de, D>(deserializer: D) -> Result<HashMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    let env = Option::<serde_json::Map<String, Value>>::deserialize(deserializer)?;
    Ok(env
        .into_iter()
        .flat_map(|env| {
            env.into_iter()
                .filter_map(|(key, value)| value.as_str().map(|value| (key, value.to_owned())))
        })
        .collect())
}

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
    background: Option<Arc<crate::background::BackgroundTaskRegistry>>,
}

impl ShellTool {
    pub fn new(
        workspace_root: Option<PathBuf>,
        sandbox_driver: Option<Arc<dyn SandboxDriver>>,
        launcher: ShellLauncher,
        bash_path: Option<PathBuf>,
    ) -> Self {
        Self {
            executor: ShellExecutor::new(workspace_root, sandbox_driver, launcher, bash_path)
                .with_output_limit_bytes(SHELL_CAPTURE_LIMIT_BYTES),
            background: None,
        }
    }

    /// Attach the shared background-task registry (enables `background=true`).
    pub fn with_background(
        mut self,
        registry: Arc<crate::background::BackgroundTaskRegistry>,
    ) -> Self {
        self.background = Some(registry);
        self
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new(None, None, ShellLauncher::default(), None)
    }
}

#[async_trait]
impl TypedTool for ShellTool {
    type Input = ShellArgs;
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return stdout, stderr, exit_code, and timeout status. \
         FOREGROUND (default): waits for completion; on timeout the process tree \
         is killed and exit_code is 124; backgrounded children (&) do NOT \
         survive the call. background=true: starts a DETACHED task instead — \
         returns immediately with a task_id; output streams to files; poll \
         task_status/task_output and stop with task_stop. Use background only \
         for long-running servers/watchers."
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

    async fn execute(&self, ctx: &ToolContext, args: ShellArgs) -> Result<ToolOutput, AgentError> {
        // Models sometimes pretty-print tool-call JSON with the command value
        // starting (or ending) on its own line, e.g. {"command": "\nls -la"}.
        // `bash -c $'\nls'` then treats the command as starting on line 2 and
        // reports `line 2: syntax error` for perfectly valid single-line
        // commands — trim both ends before execution. (Fixing this at the tool
        // layer covers every producer: streaming increments, non-streaming
        // calls, and subagents.)
        let command = args.command.trim().to_string();
        if command.is_empty() {
            return Err(AgentError::ToolExecution("'command' is empty".into()));
        }
        let timeout_secs = args.timeout_secs;
        let env = args.env;

        // Detached background variant: spawn with file-redirected stdio,
        // register the task, and return the handle immediately. The turn
        // ends; the task stays resident (dev servers, watchers).
        if args.background {
            let registry = self.background.as_ref().ok_or_else(|| {
                AgentError::ToolExecution(
                    "background execution is not available in this configuration".into(),
                )
            })?;
            let workspace_root = ctx.workspace.as_ref().map(|workspace| workspace.root.as_path());
            let task_id = registry.alloc_task_id();
            let output_dir = crate::background::BackgroundTaskRegistry::output_dir(
                workspace_root,
                &ctx.thread_id,
                &task_id,
            );
            tokio::fs::create_dir_all(&output_dir).await.map_err(|error| {
                AgentError::ToolExecution(format!(
                    "failed to create background output dir {}: {error}",
                    output_dir.display()
                ))
            })?;
            let open_log = |name: &str| {
                // CREATE/TRUNCATE, not append: each task owns a fresh
                // directory (task_id-unique), so the semantics are identical —
                // and MSYS bash (Git Bash) exits 1 with no output when it
                // inherits an APPEND-mode stdout handle on Windows
                // (msys-2.0.dll's std-handle setup rejects it).
                std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(output_dir.join(name))
                    .map_err(|error| {
                        AgentError::ToolExecution(format!(
                            "failed to open background {name} log: {error}"
                        ))
                    })
            };
            let stdout = open_log("stdout.log")?;
            let stderr = open_log("stderr.log")?;
            let child = self
                .executor
                .execute_background(
                    ShellCommand { command: command.clone(), timeout_secs, env },
                    stdout,
                    stderr,
                )
                .await
                .map_err(|e| AgentError::ToolExecution(e.to_string()))?;
            let snapshot = registry.register(
                task_id,
                ctx.thread_id.clone(),
                command.clone(),
                workspace_root,
                child,
            )?;
            return Ok(ToolOutput {
                content: serde_json::json!({
                    "background": true,
                    "task_id": snapshot.task_id,
                    "pid": snapshot.pid,
                    "status": snapshot.status.as_str(),
                    "stdout_path": snapshot.stdout_ref,
                    "stderr_path": snapshot.stderr_ref,
                    "hint": "output streams to the log files; poll task_status / task_output (tail) and stop with task_stop",
                })
                .to_string(),
                metadata: None,
            });
        }

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

        // Context budget: streams above the injection budget spill the full
        // (capture-bounded) output to a workspace artifact first, then the
        // context string is head/tail truncated with an explicit marker. The
        // envelope keeps exit_code/timed_out top-level — the turn loop's
        // exit-code sniff parses this JSON.
        let stdout_artifact =
            spill_stream(ctx, "stdout", output.stdout_bytes, output.stdout.as_str()).await;
        let stderr_artifact =
            spill_stream(ctx, "stderr", output.stderr_bytes, output.stderr.as_str()).await;

        let stdout_for_context = bound_stream_for_context(&output.stdout, output.stdout_bytes);
        let stderr_for_context = bound_stream_for_context(&output.stderr, output.stderr_bytes);

        let mut envelope = serde_json::json!({
            "stdout": stdout_for_context,
            "stderr": stderr_for_context,
            "exit_code": output.exit_code,
            "timed_out": output.timed_out,
            "stdout_bytes": output.stdout_bytes,
            "stderr_bytes": output.stderr_bytes,
        });
        if let Some(reference) = stdout_artifact {
            envelope["stdout_artifact"] = serde_json::json!(reference);
        }
        if let Some(reference) = stderr_artifact {
            envelope["stderr_artifact"] = serde_json::json!(reference);
        }

        Ok(ToolOutput { content: envelope.to_string(), metadata: None })
    }
}

/// Spill one oversized stream to `<workspace>/.slab/artifacts/<thread>/` and
/// return the workspace-relative reference (best-effort: `None` without a
/// workspace or on write failure).
async fn spill_stream(
    ctx: &ToolContext,
    stream: &str,
    stream_bytes: usize,
    content: &str,
) -> Option<String> {
    if stream_bytes <= DEFAULT_OUTPUT_LIMIT_BYTES {
        return None;
    }
    crate::artifact::write_tool_artifact(
        ctx.workspace.as_ref().map(|workspace| workspace.root.as_path()),
        &ctx.thread_id,
        &format!("shell-{stream}-t{}.txt", ctx.turn_index),
        content.as_bytes(),
    )
    .await
}

/// Apply the context-injection budget to one stream (head/tail kept, middle
/// omitted with a marker; under the budget the stream passes verbatim).
fn bound_stream_for_context(stream: &str, stream_bytes: usize) -> String {
    if stream_bytes <= DEFAULT_OUTPUT_LIMIT_BYTES {
        return stream.to_owned();
    }
    truncate_middle_bytes(stream, DEFAULT_OUTPUT_LIMIT_BYTES, CONTEXT_STREAM_HEAD_RATIO).0
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

    fn temp_workspace_ctx(name: &str, thread: &str) -> (std::path::PathBuf, ToolContext) {
        let nonce =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "slab_shell_spill_{name}_{}_{}",
            std::process::id(),
            nonce
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        let ctx = ToolContext::for_thread(thread)
            .workspace(slab_agent::WorkspaceRef { root: root.clone(), session_id: None })
            .build();
        (root, ctx)
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
        match ToolHandler::render_turn_item(&tool, &render) {
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

    /// Oversized stdout: the context string is bounded to the 30KB budget
    /// (head+tail) while the FULL output spills to a workspace artifact whose
    /// reference rides the envelope.
    #[tokio::test]
    async fn shell_tool_spills_oversized_output_to_artifact() {
        let stdout: String =
            (0..5_000).map(|idx| format!("line-{idx:04}\n")).collect::<Vec<_>>().join("");
        let stdout_bytes = stdout.len();
        let seen = Arc::new(Mutex::new(None));
        let (root, ctx) = temp_workspace_ctx("spill", "spill-thread");
        let tool = ShellTool::new(
            Some(root.clone()),
            Some(Arc::new(RecordingDriver {
                seen: Arc::clone(&seen),
                output: SandboxedOutput {
                    stdout: stdout.into_bytes(),
                    stderr: Vec::new(),
                    exit_code: 0,
                    timed_out: false,
                },
            })),
            ShellLauncher::PowerShell,
            None,
        );

        let output = ToolHandler::execute(&tool, &ctx, &json!({"command": "huge"}))
            .await
            .expect("shell output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        let stdout_in_context = value["stdout"].as_str().expect("stdout");
        assert!(
            stdout_in_context.len() < DEFAULT_OUTPUT_LIMIT_BYTES + 256,
            "context stdout not bounded: {}",
            stdout_in_context.len()
        );
        assert!(stdout_in_context.contains("line-0000"), "head must survive");
        assert!(stdout_in_context.contains("line-4999"), "tail must survive");
        assert!(stdout_in_context.contains("bytes omitted"), "marker missing");
        assert_eq!(value["stdout_bytes"], stdout_bytes);
        assert_eq!(value["exit_code"], 0, "exit_code stays top-level for the loop sniff");

        let reference = value["stdout_artifact"].as_str().expect("artifact reference on envelope");
        assert_eq!(reference, ".slab/artifacts/spill-thread/shell-stdout-t0.txt");
        let spilled = std::fs::read(
            root.join(".slab").join("artifacts").join("spill-thread").join("shell-stdout-t0.txt"),
        )
        .expect("spilled artifact exists");
        assert_eq!(spilled.len(), stdout_bytes, "artifact holds the full output");

        let _ = std::fs::remove_dir_all(root);
    }

    /// Without a workspace the spill is skipped silently — the output stays
    /// bounded either way.
    #[tokio::test]
    async fn shell_tool_without_workspace_skips_spill_silently() {
        let stdout = "x".repeat(64 * 1024);
        let tool = ShellTool::new(
            None,
            Some(Arc::new(RecordingDriver {
                seen: Arc::new(Mutex::new(None)),
                output: SandboxedOutput {
                    stdout: stdout.into_bytes(),
                    stderr: Vec::new(),
                    exit_code: 0,
                    timed_out: false,
                },
            })),
            ShellLauncher::PowerShell,
            None,
        );

        let output = ToolHandler::execute(&tool, &ctx(), &json!({"command": "huge"}))
            .await
            .expect("shell output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");
        assert!(value["stdout_artifact"].is_null(), "no artifact without a workspace");
        assert!(value["stdout"].as_str().expect("stdout").len() < DEFAULT_OUTPUT_LIMIT_BYTES + 256);
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

        let output = ToolHandler::execute(
            &tool,
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

        let missing =
            ToolHandler::execute(&tool, &ctx(), &json!({})).await.expect_err("missing command");
        assert_eq!(missing.to_string(), "tool execution error: missing 'command' argument");

        let dangerous = ToolHandler::execute(&tool, &ctx(), &json!({"command": "rm -rf /"}))
            .await
            .expect_err("dangerous command");
        assert!(dangerous.to_string().contains("command blocked"));
    }

    /// P8 regression: leading/trailing newlines around the command (from
    /// pretty-printed tool-call JSON) must be trimmed so `bash -c` does not
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

        ToolHandler::execute(&tool, &ctx(), &json!({"command": "\n  echo trimmed  \n"}))
            .await
            .expect("execute");

        let command = seen.lock().unwrap().clone().expect("driver command");
        assert_eq!(command.argv.last().map(String::as_str), Some("echo trimmed"));
    }

    #[tokio::test]
    async fn shell_tool_rejects_blank_command() {
        let tool = ShellTool::new(None, None, ShellLauncher::default(), None);

        let blank = ToolHandler::execute(&tool, &ctx(), &json!({"command": "  \n "}))
            .await
            .expect_err("blank");
        assert!(blank.to_string().contains("'command' is empty"));
    }

    #[test]
    fn shell_tool_describes_operation_as_shell_command() {
        let tool = ShellTool::new(None, None, ShellLauncher::default(), None);
        let descriptor = ToolHandler::describe_operation(&tool, &json!({"command": "cargo check"}))
            .expect("descriptor");
        assert_eq!(descriptor.category, slab_agent::OperationCategory::Shell);
        assert_eq!(descriptor.subject, "cargo check");
    }

    #[test]
    fn shell_tool_schema_marks_command_required() {
        let schema = ToolHandler::parameters_schema(&ShellTool::default());

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
            ToolHandler::execute(&tool, &ctx, &args).await
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
            ToolHandler::execute(&tool, &ctx, &args).await
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

    /// Harness-review regression: `2>/dev/null` — the most common shell
    /// silencing idiom — must survive the FULL production path (guard ->
    /// platform sandbox driver -> bash). The guard used to join the target
    /// with the cwd (`C:\…\dev\null`) and refuse the command outright.
    #[tokio::test]
    async fn shell_tool_runs_dev_null_redirection_through_platform_driver() {
        use slab_sandboxing::{SandboxEnvironment, SandboxPolicy, create_platform_driver};

        let workspace = PathBuf::from(".");
        let env = SandboxEnvironment::new(Some(workspace.clone()), SandboxPolicy::WorkspaceWrite);
        let driver = create_platform_driver(env).expect("platform driver");
        if !driver.setup_status().available {
            eprintln!("skipping: platform driver unavailable");
            return;
        }

        let tool = ShellTool::new(Some(workspace), Some(driver), ShellLauncher::Auto, None);
        let output = ToolHandler::execute(
            &tool,
            &ctx(),
            &json!({ "command": "echo dev-null-marker 2>/dev/null" }),
        )
        .await
        .expect("dev-null redirection run");
        let value: Value = serde_json::from_str(&output.content).expect("json");
        assert_eq!(value["exit_code"], 0, "stderr: {}", value["stderr"]);
        assert!(value["stdout"].as_str().unwrap_or("").contains("dev-null-marker"));
    }

    /// Background contract: `shell background=true` returns immediately with a
    /// task handle, a finished task's output is readable via `task_output`,
    /// a running task survives the tool-call return, and `task_stop`
    /// terminates it. (Buffering note: shells block-buffer stdout to files —
    /// output lands on flush/exit, which is why the output assertion uses a
    /// self-terminating command and the residency assertion needs none.)
    #[tokio::test]
    async fn shell_background_returns_handle_output_and_stops() {
        let (root, ctx) = temp_workspace_ctx("background", "bg-thread");
        let registry = Arc::new(crate::background::BackgroundTaskRegistry::default());
        let tool = ShellTool::new(Some(root.clone()), None, ShellLauncher::default(), None)
            .with_background(Arc::clone(&registry));

        // Phase 0 — control: the same executor in the foreground must work.
        let output =
            ToolHandler::execute(&tool, &ctx, &json!({ "command": "echo fg-control-marker" }))
                .await
                .expect("foreground control");
        eprintln!("FG CONTROL: {}", output.content);

        // Phase 1 — output contract: a self-terminating writer.
        let started = std::time::Instant::now();
        let output = ToolHandler::execute(
            &tool,
            &ctx,
            &json!({ "command": "echo bg-done-marker", "background": true }),
        )
        .await
        .expect("background spawn");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "background spawn must return immediately, took {:?}",
            started.elapsed()
        );
        let value: Value = serde_json::from_str(&output.content).expect("json");
        assert_eq!(value["background"], true);
        let writer_id = value["task_id"].as_str().expect("task id").to_owned();
        let stdout_path = value["stdout_path"].as_str().expect("stdout ref").to_owned();
        assert!(stdout_path.contains("background"), "log under the background dir: {stdout_path}");

        // The writer exits on its own; poll task_status until terminal.
        let status_tool = crate::background::TaskStatusTool::new(Arc::clone(&registry));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut writer_status = String::from("running");
        while std::time::Instant::now() < deadline {
            let output = ToolHandler::execute(&status_tool, &ctx, &json!({ "task_id": writer_id }))
                .await
                .expect("status");
            let value: Value = serde_json::from_str(&output.content).expect("json");
            writer_status = value["task"]["status"].as_str().unwrap_or("").to_owned();
            if writer_status != "running" {
                let stderr_dump = std::fs::read_to_string(
                    root.join(".slab")
                        .join("artifacts")
                        .join("bg-thread")
                        .join("background")
                        .join(&writer_id)
                        .join("stderr.log"),
                )
                .unwrap_or_else(|e| format!("<stderr read failed: {e}>"));
                assert_eq!(
                    value["task"]["exit_code"], 0,
                    "echo must exit cleanly; stderr: {stderr_dump:?}"
                );
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        assert_ne!(writer_status, "running", "writer must terminate on its own");

        let output_tool = crate::background::TaskOutputTool::new(Arc::clone(&registry));
        let output = ToolHandler::execute(&output_tool, &ctx, &json!({ "task_id": writer_id }))
            .await
            .expect("tail");
        let value: Value = serde_json::from_str(&output.content).expect("json");
        assert!(
            value["output"].as_str().unwrap_or("").contains("bg-done-marker"),
            "log must carry the output: {value}"
        );

        // Phase 2 — residency + stop: a long runner survives the tool-call
        // return and dies on task_stop.
        let output = ToolHandler::execute(
            &tool,
            &ctx,
            &json!({ "command": "sleep 30", "background": true }),
        )
        .await
        .expect("background spawn 2");
        let value: Value = serde_json::from_str(&output.content).expect("json");
        let sleeper_id = value["task_id"].as_str().expect("task id").to_owned();

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let output = ToolHandler::execute(&status_tool, &ctx, &json!({ "task_id": sleeper_id }))
            .await
            .expect("status 2");
        let value: Value = serde_json::from_str(&output.content).expect("json");
        assert_eq!(
            value["task"]["status"], "running",
            "the task must outlive the tool call that started it"
        );

        let stop_tool = crate::background::TaskStopTool::new(Arc::clone(&registry));
        let output = ToolHandler::execute(&stop_tool, &ctx, &json!({ "task_id": sleeper_id }))
            .await
            .expect("stop");
        let value: Value = serde_json::from_str(&output.content).expect("json");
        assert_eq!(value["stopped"]["status"], "stopped");

        let _ = std::fs::remove_dir_all(root);
    }

    /// Background without a registry attached (legacy/test wiring) is a clear
    /// error, not a silent foreground fallthrough.
    #[tokio::test]
    async fn shell_background_without_registry_errors() {
        let tool = ShellTool::default();
        let error = ToolHandler::execute(
            &tool,
            &ctx(),
            &json!({ "command": "echo hi", "background": true }),
        )
        .await
        .expect_err("no registry");
        assert!(error.to_string().contains("not available"));
    }
}
