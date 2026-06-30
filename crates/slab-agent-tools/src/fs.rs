//! File-system read/write/list tools.

use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{AgentError, ToolApprovalRequest, ToolContext, ToolHandler, ToolOutput};

use crate::{args::string_arg, sensitive_path::approval_for_path};

const MAX_LINES: usize = 1000;

pub struct ReadFileTool {
    pub workspace_root: Option<PathBuf>,
    pub extra_roots: Vec<PathBuf>,
}

impl ReadFileTool {
    pub fn new(workspace_root: Option<PathBuf>) -> Self {
        Self { workspace_root, extra_roots: Vec::new() }
    }

    pub fn new_with_extra_roots(
        workspace_root: Option<PathBuf>,
        extra_roots: Vec<PathBuf>,
    ) -> Self {
        Self { workspace_root, extra_roots }
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

    fn approval_request(&self, arguments: &Value) -> Option<ToolApprovalRequest> {
        approval_for_path("read_file", "path", arguments.get("path").and_then(Value::as_str))
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let path = string_arg(arguments, "path")?;
        let start_line = arguments.get("start_line").and_then(Value::as_u64).unwrap_or(1) as usize;
        let end_line = arguments.get("end_line").and_then(Value::as_u64).map(|v| v as usize);
        let path = resolve_agent_path(self.workspace_root.as_deref(), &self.extra_roots, path)?;
        let raw = tokio::fs::read_to_string(path)
            .await
            .map_err(|error| AgentError::ToolExecution(error.to_string()))?;

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
    pub extra_roots: Vec<PathBuf>,
}

impl WriteFileTool {
    pub fn new(workspace_root: Option<PathBuf>) -> Self {
        Self { workspace_root, extra_roots: Vec::new() }
    }

    pub fn new_with_extra_roots(
        workspace_root: Option<PathBuf>,
        extra_roots: Vec<PathBuf>,
    ) -> Self {
        Self { workspace_root, extra_roots }
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
        let requested_path = string_arg(arguments, "path")?;
        let content = string_arg(arguments, "content")?;
        let path =
            resolve_agent_path(self.workspace_root.as_deref(), &self.extra_roots, requested_path)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| AgentError::ToolExecution(error.to_string()))?;
        }
        tokio::fs::write(&path, content)
            .await
            .map_err(|error| AgentError::ToolExecution(error.to_string()))?;

        Ok(ToolOutput {
            content: serde_json::json!({
                "written": requested_path,
                "bytes": content.len()
            })
            .to_string(),
            metadata: None,
        })
    }
}

pub struct ListDirTool {
    pub workspace_root: Option<PathBuf>,
    pub extra_roots: Vec<PathBuf>,
}

impl ListDirTool {
    pub fn new(workspace_root: Option<PathBuf>) -> Self {
        Self { workspace_root, extra_roots: Vec::new() }
    }

    pub fn new_with_extra_roots(
        workspace_root: Option<PathBuf>,
        extra_roots: Vec<PathBuf>,
    ) -> Self {
        Self { workspace_root, extra_roots }
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

    fn approval_request(&self, arguments: &Value) -> Option<ToolApprovalRequest> {
        approval_for_path("list_dir", "path", arguments.get("path").and_then(Value::as_str))
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let path = string_arg(arguments, "path")?;
        let path = resolve_agent_path(self.workspace_root.as_deref(), &self.extra_roots, path)?;
        let entries =
            slab_file::list_dir(None, &path.to_string_lossy()).await.map_err(to_tool_error)?;

        Ok(ToolOutput {
            content: serde_json::json!({ "entries": entries }).to_string(),
            metadata: None,
        })
    }
}

fn to_tool_error(error: slab_file::FileSystemError) -> AgentError {
    AgentError::ToolExecution(error.to_string())
}

pub(crate) fn resolve_agent_path(
    workspace_root: Option<&std::path::Path>,
    extra_roots: &[PathBuf],
    path: &str,
) -> Result<PathBuf, AgentError> {
    let path_buf = PathBuf::from(path);
    if path_buf.is_absolute() {
        if path_is_under_extra_root(&path_buf, extra_roots) {
            return Ok(path_buf);
        }
        return slab_file::resolve_path(workspace_root, path).map_err(to_tool_error);
    }
    slab_file::resolve_path(workspace_root, path).map_err(to_tool_error)
}

fn path_is_under_extra_root(path: &std::path::Path, extra_roots: &[PathBuf]) -> bool {
    let Ok(candidate_parent) = slab_utils::fs::existing_ancestor(path.parent().unwrap_or(path))
    else {
        return false;
    };
    extra_roots.iter().any(|root| {
        root.canonicalize()
            .map(|canonical_root| candidate_parent.starts_with(canonical_root))
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::{Value, json};
    use slab_agent::ToolHandler;

    use super::*;

    fn ctx() -> ToolContext {
        ToolContext::for_thread("thread").build()
    }

    #[tokio::test]
    async fn read_file_tool_respects_line_ranges_and_reports_truncation() {
        let root = temp_root("read_range");
        fs::write(root.join("notes.txt"), "one\ntwo\nthree\n").expect("seed file");
        let tool = ReadFileTool::new(Some(root.clone()));

        let output = tool
            .execute(&ctx(), &json!({"path": "notes.txt", "start_line": 2, "end_line": 3}))
            .await
            .expect("read file");
        let value: Value = serde_json::from_str(&output.content).expect("json output");
        assert_eq!(value["content"], "two\nthree");
        assert_eq!(value["total_lines"], 3);
        assert_eq!(value["returned_lines"], 2);
        assert_eq!(value["truncated"], false);

        let output = tool
            .execute(&ctx(), &json!({"path": "notes.txt", "start_line": 2_000}))
            .await
            .expect("out of range read");
        let value: Value = serde_json::from_str(&output.content).expect("json output");
        assert_eq!(value["content"], "");
        assert_eq!(value["returned_lines"], 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn read_and_list_tools_require_approval_for_sensitive_paths() {
        let read = ReadFileTool::new(Some(PathBuf::from(".")));
        let list = ListDirTool::new(Some(PathBuf::from(".")));

        assert!(read.approval_request(&json!({"path": ".env"})).is_some());
        assert!(read.approval_request(&json!({"path": ".slab/slab.db"})).is_some());
        assert!(list.approval_request(&json!({"path": "~/.ssh"})).is_some());
        assert!(read.approval_request(&json!({"path": "src/main.rs"})).is_none());
    }

    #[tokio::test]
    async fn write_and_list_tools_stay_inside_workspace() {
        let root = temp_root("write_list");
        let write = WriteFileTool::new(Some(root.clone()));
        let list = ListDirTool::new(Some(root.clone()));

        let output = write
            .execute(&ctx(), &json!({"path": "dir/note.txt", "content": "hello"}))
            .await
            .expect("write file");
        let value: Value = serde_json::from_str(&output.content).expect("json output");
        assert_eq!(value["written"], "dir/note.txt");
        assert_eq!(value["bytes"], 5);
        assert_eq!(fs::read_to_string(root.join("dir").join("note.txt")).unwrap(), "hello");

        let output = list.execute(&ctx(), &json!({"path": "dir"})).await.expect("list dir");
        let value: Value = serde_json::from_str(&output.content).expect("json output");
        assert_eq!(value["entries"].as_array().expect("entries").len(), 1);
        assert_eq!(value["entries"][0]["name"], "note.txt");

        let error = write
            .execute(&ctx(), &json!({"path": "../outside.txt", "content": "nope"}))
            .await
            .expect_err("escape rejected");
        assert!(error.to_string().contains("workspace path `../outside.txt` is invalid"));

        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "slab_agent_tools_fs_{name}_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }
}
