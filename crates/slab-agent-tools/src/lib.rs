//! Built-in tool implementations for the Slab agent runtime.
//!
//! `slab-agent` owns the orchestration kernel and tool traits. This crate
//! contains host-provided deterministic tools and registration helpers.

use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput, ToolRouter};

pub mod fs;
pub mod fs_watch;
pub mod grep;
pub mod shell;

pub use fs::{ListDirTool, ReadFileTool, WriteFileTool};
pub use fs_watch::FsWatchTool;
pub use grep::GrepTool;
pub use shell::{ShellPolicy, ShellTool};

/// Register all built-in host tools with the provided router.
///
/// Registers: echo, shell, read_file, write_file, list_dir, grep, fs_watch.
pub fn register_builtin_tools(router: &mut ToolRouter) {
    router.register(Box::new(EchoTool));
}

/// Register the full production tool suite.
///
/// - `shell_policy`: controls whether shell commands are allowed or blocked.
/// - `workspace_root`: optional root directory for file/shell tools.
pub fn register_all_tools(
    router: &mut ToolRouter,
    shell_policy: ShellPolicy,
    workspace_root: Option<PathBuf>,
) {
    router.register(Box::new(EchoTool));
    router.register(Box::new(ShellTool::new(shell_policy, workspace_root.clone())));
    router.register(Box::new(ReadFileTool));
    router.register(Box::new(WriteFileTool));
    router.register(Box::new(ListDirTool));
    router.register(Box::new(GrepTool::new(workspace_root)));
    if let Some(watcher) = FsWatchTool::new() {
        router.register(Box::new(watcher));
    }
}

/// A trivial tool that echoes its `message` argument back to the agent.
pub struct EchoTool;

#[async_trait]
impl ToolHandler for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echo the provided message back verbatim. Useful for testing the agent tool-call loop."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The text to echo back."
                }
            },
            "required": ["message"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let message = arguments.get("message").and_then(Value::as_str).unwrap_or("").to_owned();
        Ok(ToolOutput { content: message, metadata: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn echo_tool_returns_input() {
        let ctx = ToolContext { thread_id: "t1".into(), turn_index: 0, depth: 0 };
        let args = serde_json::json!({"message": "test message"});

        let output = EchoTool.execute(&ctx, &args).await.expect("echo should succeed");
        assert_eq!(output.content, "test message");
    }

    #[tokio::test]
    async fn echo_tool_missing_message_returns_empty() {
        let ctx = ToolContext { thread_id: "t1".into(), turn_index: 0, depth: 0 };
        let args = serde_json::json!({});

        let output = EchoTool.execute(&ctx, &args).await.expect("echo should succeed");
        assert_eq!(output.content, "");
    }

    #[test]
    fn register_builtin_tools_adds_echo() {
        let mut router = ToolRouter::new();
        register_builtin_tools(&mut router);

        let specs = router.tool_specs();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name, "echo");
    }
}
