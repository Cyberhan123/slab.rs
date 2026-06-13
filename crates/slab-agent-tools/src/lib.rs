//! Built-in tool implementations for the Slab agent runtime.
//!
//! `slab-agent` owns the orchestration kernel and tool traits. This crate
//! contains host-provided deterministic tools and registration helpers.

use std::{path::PathBuf, sync::Arc};

use slab_agent::{ToolHandler, ToolRouter};
use slab_config::AgentWebSearchConfig;
use slab_mcp::McpClient;
use slab_sandboxing::SandboxDriver;

pub mod apply_patch;
mod args;
pub mod fs;
pub mod fs_watch;
pub mod git;
pub mod glob;
pub mod grep;
pub mod mcp;
pub mod plan;
pub mod shell;
pub mod subagent;
pub mod web_search;

pub use apply_patch::ApplyPatchTool;
pub use fs::{ListDirTool, ReadFileTool, WriteFileTool};
pub use fs_watch::FsWatchTool;
pub use git::{GitCommitTool, GitDiffTool, GitStatusTool};
pub use glob::FileGlobTool;
pub use grep::GrepTool;
pub use mcp::{McpCallTool, McpListToolsTool, McpProxyTool};
pub use plan::PlanUpdateTool;
pub use shell::{ShellPolicy, ShellTool};
pub use slab_shell_command::{
    ShellRule, ShellRuleAction, ShellRuleError, ShellRuleMatcher, ShellRuleSet,
};
pub use subagent::DelegateSubagentTool;
pub use web_search::WebSearchTool;

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
    register_all_tools_with_shell_rules(
        router,
        shell_policy,
        sandbox_driver,
        workspace_root,
        mcp_client,
        git_tools,
        web_search_config,
        ShellRuleSet::default(),
    );
}

/// Register the full production tool suite with command-specific shell rules.
#[allow(clippy::too_many_arguments)]
pub fn register_all_tools_with_shell_rules(
    router: &mut ToolRouter,
    shell_policy: ShellPolicy,
    sandbox_driver: Option<Arc<dyn SandboxDriver>>,
    workspace_root: Option<PathBuf>,
    mcp_client: Option<Arc<McpClient>>,
    git_tools: bool,
    web_search_config: AgentWebSearchConfig,
    shell_rules: ShellRuleSet,
) {
    router.register(Box::new(ShellTool::new_with_rules(
        shell_policy,
        workspace_root.clone(),
        sandbox_driver,
        shell_rules,
    )));
    router.register(Box::new(ReadFileTool::new(workspace_root.clone())));
    router.register(Box::new(WriteFileTool::new(workspace_root.clone())));
    router.register(Box::new(ListDirTool::new(workspace_root.clone())));
    router.register(Box::new(FileGlobTool::new(workspace_root.clone())));
    router.register(Box::new(GrepTool::new(workspace_root.clone())));
    router.register(Box::new(PlanUpdateTool::new()));
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
        router.register(Box::new(McpListToolsTool::new(Arc::clone(&client))));
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(router.get("file_glob").is_some());
        assert!(router.get("plan_update").is_some());
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
        assert!(router.get("file_glob").is_some());
        assert!(router.get("plan_update").is_some());
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
