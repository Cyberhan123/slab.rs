//! Git tools backed by `slab-git`.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use slab_agent::{
    AgentError, ToolContext, ToolHandler, ToolOutput, parse_tool_input, typed_input_schema,
};
use slab_git::GitRepository;
use slab_sandboxing::SandboxDriver;

/// Arguments for the `git_diff` tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct GitDiffArgs {
    /// Optional relative path to diff.
    path: Option<String>,
    /// Return staged changes when true.
    #[serde(default)]
    staged: bool,
}

/// Arguments for the `git_commit` tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct GitCommitArgs {
    /// Commit message.
    message: String,
}

pub struct GitStatusTool {
    workspace_root: PathBuf,
    sandbox_driver: Option<Arc<dyn SandboxDriver>>,
}

impl GitStatusTool {
    pub fn new(workspace_root: PathBuf, sandbox_driver: Option<Arc<dyn SandboxDriver>>) -> Self {
        Self { workspace_root, sandbox_driver }
    }
}

#[async_trait]
impl ToolHandler for GitStatusTool {
    fn name(&self) -> &str {
        "git_status"
    }

    /// Pure read — safe to run concurrently with other read-only calls.
    fn is_concurrency_safe(&self, _arguments: &serde_json::Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Return the current Git status for the configured workspace."
    }

    fn parameters_schema(&self) -> Value {
        // No-arg tool: `Value` keeps any stray arguments tolerated at parse
        // time (an empty struct would reject non-object calls).
        typed_input_schema::<Value>()
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        _arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let status =
            GitRepository::new_with_driver(&self.workspace_root, self.sandbox_driver.clone())
                .status()
                .await
                .map_err(to_tool_error)?;
        Ok(json_output(&status)?)
    }
}

pub struct GitDiffTool {
    workspace_root: PathBuf,
    sandbox_driver: Option<Arc<dyn SandboxDriver>>,
}

impl GitDiffTool {
    pub fn new(workspace_root: PathBuf, sandbox_driver: Option<Arc<dyn SandboxDriver>>) -> Self {
        Self { workspace_root, sandbox_driver }
    }
}

#[async_trait]
impl ToolHandler for GitDiffTool {
    fn name(&self) -> &str {
        "git_diff"
    }

    /// Pure read — safe to run concurrently with other read-only calls.
    fn is_concurrency_safe(&self, _arguments: &serde_json::Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Return a staged or unstaged Git diff for the configured workspace."
    }

    fn parameters_schema(&self) -> Value {
        typed_input_schema::<GitDiffArgs>()
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let args = parse_tool_input::<GitDiffArgs>(arguments)?;
        let diff =
            GitRepository::new_with_driver(&self.workspace_root, self.sandbox_driver.clone())
                .diff(args.path.as_deref(), args.staged)
                .await
                .map_err(to_tool_error)?;
        Ok(json_output(&diff)?)
    }
}

pub struct GitCommitTool {
    workspace_root: PathBuf,
    sandbox_driver: Option<Arc<dyn SandboxDriver>>,
}

impl GitCommitTool {
    pub fn new(workspace_root: PathBuf, sandbox_driver: Option<Arc<dyn SandboxDriver>>) -> Self {
        Self { workspace_root, sandbox_driver }
    }
}

#[async_trait]
impl ToolHandler for GitCommitTool {
    fn name(&self) -> &str {
        "git_commit"
    }

    fn description(&self) -> &str {
        "Stage all workspace changes and commit them with the provided message."
    }

    fn parameters_schema(&self) -> Value {
        typed_input_schema::<GitCommitArgs>()
    }

    fn describe_operation(&self, arguments: &Value) -> Option<slab_agent::OperationDescriptor> {
        let message = arguments.get("message").and_then(Value::as_str)?;
        Some(
            slab_agent::OperationDescriptor::file_edit(format!("git_commit: {message}"))
                .with_workspace(Some(self.workspace_root.clone())),
        )
    }

    fn category(&self) -> slab_agent::OperationCategory {
        slab_agent::OperationCategory::FileEdit
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let args = parse_tool_input::<GitCommitArgs>(arguments)?;
        let result =
            GitRepository::new_with_driver(&self.workspace_root, self.sandbox_driver.clone())
                .commit_all(&args.message)
                .await
                .map_err(to_tool_error)?;
        Ok(json_output(&result)?)
    }
}

fn json_output<T: serde::Serialize>(value: &T) -> Result<ToolOutput, AgentError> {
    Ok(ToolOutput {
        content: serde_json::to_string(value)
            .map_err(|error| AgentError::ToolExecution(error.to_string()))?,
        metadata: None,
    })
}

fn to_tool_error(error: slab_git::GitError) -> AgentError {
    AgentError::ToolExecution(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::{Value, json};
    use slab_agent::ToolHandler;

    use super::*;

    fn ctx() -> ToolContext {
        ToolContext::for_thread("thread").build()
    }

    #[test]
    fn git_commit_describes_file_edit_operation() {
        let tool = GitCommitTool::new(PathBuf::from("."), None);

        let desc =
            tool.describe_operation(&json!({"message": "fix quoted path"})).expect("descriptor");

        assert_eq!(desc.category, slab_agent::OperationCategory::FileEdit);
        assert_eq!(desc.subject, "git_commit: fix quoted path");
        assert!(tool.describe_operation(&json!({"message": false})).is_none());
    }

    #[tokio::test]
    async fn git_commit_requires_message_before_touching_repository() {
        let tool = GitCommitTool::new(PathBuf::from("missing-workspace"), None);

        let error = tool.execute(&ctx(), &json!({})).await.expect_err("missing message");

        assert_eq!(error.to_string(), "tool execution error: missing 'message' argument");
    }

    #[tokio::test]
    async fn git_diff_rejects_escape_and_git_internal_paths() {
        let root = temp_root("diff_paths");
        let tool = GitDiffTool::new(root.clone(), None);

        let escape = tool
            .execute(&ctx(), &json!({"path": "../outside.txt"}))
            .await
            .expect_err("parent escape rejected");
        assert!(escape.to_string().contains("invalid path"));

        let internal = tool
            .execute(&ctx(), &json!({"path": ".git/config"}))
            .await
            .expect_err("git internals rejected");
        assert!(internal.to_string().contains("Git internals cannot be edited"));

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn git_status_tool_returns_json_for_non_repository() {
        let root = temp_root("status_non_repo");
        let tool = GitStatusTool::new(root.clone(), None);

        let output = tool.execute(&ctx(), &json!({})).await.expect("status output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert!(value["available"].is_boolean());
        assert_eq!(value["entries"], json!([]));
        assert!(value["message"].is_string() || value["message"].is_null());

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn git_diff_tool_reports_untracked_file_diff_when_git_is_available() {
        let root = temp_root("diff_untracked");
        if run_git(&root, &["init"]).is_none() {
            let _ = fs::remove_dir_all(root);
            return;
        }
        fs::write(root.join("note.txt"), "hello\n").expect("write untracked file");
        let tool = GitDiffTool::new(root.clone(), None);

        let output = tool
            .execute(&ctx(), &json!({"path": "note.txt", "staged": false}))
            .await
            .expect("diff output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert_eq!(value["path"], "note.txt");
        assert_eq!(value["staged"], false);
        assert!(value["diff"].as_str().expect("diff").contains("+hello"));

        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "slab_agent_tools_git_{name}_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn run_git(root: &Path, args: &[&str]) -> Option<std::process::Output> {
        Command::new("git").arg("-C").arg(root).args(args).output().ok()
    }
}
