//! Git tools backed by `slab-git`.

use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{AgentError, ToolApprovalRequest, ToolContext, ToolHandler, ToolOutput};
use slab_git::GitRepository;

pub struct GitStatusTool {
    workspace_root: PathBuf,
}

impl GitStatusTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl ToolHandler for GitStatusTool {
    fn name(&self) -> &str {
        "git_status"
    }

    fn description(&self) -> &str {
        "Return the current Git status for the configured workspace."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        _arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let status = GitRepository::new(&self.workspace_root).status().map_err(to_tool_error)?;
        Ok(json_output(&status)?)
    }
}

pub struct GitDiffTool {
    workspace_root: PathBuf,
}

impl GitDiffTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl ToolHandler for GitDiffTool {
    fn name(&self) -> &str {
        "git_diff"
    }

    fn description(&self) -> &str {
        "Return a staged or unstaged Git diff for the configured workspace."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Optional relative path to diff."
                },
                "staged": {
                    "type": "boolean",
                    "description": "Return staged changes when true.",
                    "default": false
                }
            }
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let path = arguments.get("path").and_then(Value::as_str);
        let staged = arguments.get("staged").and_then(Value::as_bool).unwrap_or(false);
        let diff =
            GitRepository::new(&self.workspace_root).diff(path, staged).map_err(to_tool_error)?;
        Ok(json_output(&diff)?)
    }
}

pub struct GitCommitTool {
    workspace_root: PathBuf,
}

impl GitCommitTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
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
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "Commit message."
                }
            },
            "required": ["message"]
        })
    }

    fn approval_request(&self, arguments: &Value) -> Option<ToolApprovalRequest> {
        let message = arguments.get("message").and_then(Value::as_str)?;
        Some(ToolApprovalRequest { command: format!("git add --all && git commit -m {message:?}") })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let message = arguments
            .get("message")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::ToolExecution("missing 'message' argument".into()))?;
        let result =
            GitRepository::new(&self.workspace_root).commit_all(message).map_err(to_tool_error)?;
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
        ToolContext { thread_id: "thread".into(), turn_index: 0, depth: 0 }
    }

    #[test]
    fn git_commit_approval_quotes_message_for_shell_display() {
        let tool = GitCommitTool::new(PathBuf::from("."));

        let request = tool
            .approval_request(&json!({"message": "fix \"quoted\" path"}))
            .expect("approval request");

        assert_eq!(request.command, "git add --all && git commit -m \"fix \\\"quoted\\\" path\"");
        assert!(tool.approval_request(&json!({"message": false})).is_none());
    }

    #[tokio::test]
    async fn git_commit_requires_message_before_touching_repository() {
        let tool = GitCommitTool::new(PathBuf::from("missing-workspace"));

        let error = tool.execute(&ctx(), &json!({})).await.expect_err("missing message");

        assert_eq!(error.to_string(), "tool execution error: missing 'message' argument");
    }

    #[tokio::test]
    async fn git_diff_rejects_escape_and_git_internal_paths() {
        let root = temp_root("diff_paths");
        let tool = GitDiffTool::new(root.clone());

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
        let tool = GitStatusTool::new(root.clone());

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
        let tool = GitDiffTool::new(root.clone());

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
