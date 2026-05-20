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
