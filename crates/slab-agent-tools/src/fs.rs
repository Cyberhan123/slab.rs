//! File-system read/write/list tools.

use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput};

const MAX_LINES: usize = 1000;

pub struct ReadFileTool {
    pub workspace_root: Option<PathBuf>,
}

impl ReadFileTool {
    pub fn new(workspace_root: Option<PathBuf>) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl ToolHandler for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read a file, optionally restricted to a 1-based inclusive line range."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "start_line": { "type": "integer", "minimum": 1 },
                "end_line": { "type": "integer", "minimum": 1 }
            },
            "required": ["path"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let path = string_arg(arguments, "path")?;
        let start_line = arguments.get("start_line").and_then(Value::as_u64).unwrap_or(1) as usize;
        let end_line = arguments.get("end_line").and_then(Value::as_u64).map(|v| v as usize);
        let raw = slab_file_system::read_to_string(self.workspace_root.as_deref(), path)
            .await
            .map_err(to_tool_error)?;

        let start_idx = start_line.saturating_sub(1);
        let lines: Vec<&str> = raw.lines().collect();
        let total = lines.len();
        let requested_end = end_line.map(|end| end.min(total)).unwrap_or(total);
        let capped_end = requested_end.min(start_idx + MAX_LINES);
        let selected = lines.get(start_idx..capped_end).unwrap_or(&[]).to_vec();

        Ok(ToolOutput {
            content: serde_json::json!({
                "content": selected.join("\n"),
                "total_lines": total,
                "returned_lines": selected.len(),
                "truncated": capped_end < requested_end
            })
            .to_string(),
            metadata: None,
        })
    }
}

pub struct WriteFileTool {
    pub workspace_root: Option<PathBuf>,
}

impl WriteFileTool {
    pub fn new(workspace_root: Option<PathBuf>) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl ToolHandler for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file, creating parent directories when needed."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let path = string_arg(arguments, "path")?;
        let content = string_arg(arguments, "content")?;
        slab_file_system::write_string(self.workspace_root.as_deref(), path, content)
            .await
            .map_err(to_tool_error)?;

        Ok(ToolOutput {
            content: serde_json::json!({
                "written": path,
                "bytes": content.len()
            })
            .to_string(),
            metadata: None,
        })
    }
}

pub struct ListDirTool {
    pub workspace_root: Option<PathBuf>,
}

impl ListDirTool {
    pub fn new(workspace_root: Option<PathBuf>) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl ToolHandler for ListDirTool {
    fn name(&self) -> &str {
        "list_dir"
    }

    fn description(&self) -> &str {
        "List the immediate children of a directory."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "required": ["path"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let path = string_arg(arguments, "path")?;
        let entries = slab_file_system::list_dir(self.workspace_root.as_deref(), path)
            .await
            .map_err(to_tool_error)?;

        Ok(ToolOutput {
            content: serde_json::json!({ "entries": entries }).to_string(),
            metadata: None,
        })
    }
}

fn string_arg<'a>(arguments: &'a Value, name: &str) -> Result<&'a str, AgentError> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| AgentError::ToolExecution(format!("missing '{name}' argument")))
}

fn to_tool_error(error: slab_file_system::FileSystemError) -> AgentError {
    AgentError::ToolExecution(error.to_string())
}
