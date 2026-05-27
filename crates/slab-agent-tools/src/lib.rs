//! Built-in tool implementations for the Slab agent runtime.
//!
//! `slab-agent` owns the orchestration kernel and tool traits. This crate
//! contains host-provided deterministic tools and registration helpers.

use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput, ToolRouter};
use slab_config::AgentWebSearchConfig;
use slab_mcp::McpClient;
use slab_sandboxing::SandboxDriver;

pub mod apply_patch;
pub mod fs;
pub mod fs_watch;
pub mod git;
pub mod grep;
pub mod mcp;
pub mod shell;
pub mod web_search;

pub use apply_patch::ApplyPatchTool;
pub use fs::{ListDirTool, ReadFileTool, WriteFileTool};
pub use fs_watch::FsWatchTool;
pub use git::{GitCommitTool, GitDiffTool, GitStatusTool};
pub use grep::GrepTool;
pub use mcp::{McpCallTool, McpProxyTool};
pub use shell::{ShellPolicy, ShellTool};
pub use web_search::WebSearchTool;

/// Register only the minimal built-in tool (echo).
///
/// For the full production suite use [`register_all_tools`] instead.
pub fn register_builtin_tools(router: &mut ToolRouter) {
    router.register(Box::new(EchoTool));
}

/// Register the full production tool suite.
pub fn register_all_tools(
    router: &mut ToolRouter,
    shell_policy: ShellPolicy,
    sandbox_driver: Option<Arc<dyn SandboxDriver>>,
    workspace_root: Option<PathBuf>,
    mcp_client: Option<Arc<McpClient>>,
    git_tools: bool,
    web_search_config: AgentWebSearchConfig,
) {
    router.register(Box::new(EchoTool));
    router.register(Box::new(ShellTool::new(shell_policy, workspace_root.clone(), sandbox_driver)));
    router.register(Box::new(ReadFileTool::new(workspace_root.clone())));
    router.register(Box::new(WriteFileTool::new(workspace_root.clone())));
    router.register(Box::new(ListDirTool::new(workspace_root.clone())));
    router.register(Box::new(GrepTool::new(workspace_root.clone())));
    router.register(Box::new(WebSearchTool::new(web_search_config)));
    if let Some(watcher) = FsWatchTool::new() {
        router.register(Box::new(watcher));
    }
    if let Some(root) = workspace_root {
        router.register(Box::new(ApplyPatchTool::new(root.clone())));
        if git_tools {
            router.register(Box::new(GitStatusTool::new(root.clone())));
            router.register(Box::new(GitDiffTool::new(root.clone())));
            router.register(Box::new(GitCommitTool::new(root)));
        }
    }
    if let Some(client) = mcp_client {
        router.register(Box::new(McpCallTool::new(Arc::clone(&client))));
        for spec in client.cached_tools_blocking() {
            let tool = McpProxyTool::new(Arc::clone(&client), spec);
            if router.get(tool.name()).is_some() {
                tracing::warn!(tool = tool.name(), "skipping conflicting MCP proxy tool");
                continue;
            }
            router.register(Box::new(tool));
        }
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

    #[test]
    fn register_all_tools_respects_workspace_and_git_switches() {
        let mut router = ToolRouter::new();
        register_all_tools(
            &mut router,
            ShellPolicy::Block,
            None,
            None,
            None,
            true,
            AgentWebSearchConfig::default(),
        );
        assert!(router.get("shell").is_some());
        assert!(router.get("web_search").is_some());
        assert!(router.get("apply_patch").is_none());
        assert!(router.get("git_status").is_none());

        let mut router = ToolRouter::new();
        register_all_tools(
            &mut router,
            ShellPolicy::Block,
            None,
            Some(PathBuf::from(".")),
            None,
            false,
            AgentWebSearchConfig::default(),
        );
        assert!(router.get("apply_patch").is_some());
        assert!(router.get("git_status").is_none());

        let mut router = ToolRouter::new();
        register_all_tools(
            &mut router,
            ShellPolicy::Block,
            None,
            Some(PathBuf::from(".")),
            None,
            true,
            AgentWebSearchConfig::default(),
        );
        assert!(router.get("git_status").is_some());
        assert!(router.get("git_diff").is_some());
        assert!(router.get("git_commit").is_some());
    }
}
