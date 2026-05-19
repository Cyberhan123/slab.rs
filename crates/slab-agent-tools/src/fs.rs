//! File-system read/write/list tools.
//!
//! These tools give the agent the ability to read files, write files, and list
//! directory contents.  Inspired by the codex `file-system` module.
//!
//! When a `workspace_root` is configured all paths are interpreted as relative
//! to that root and are validated to remain within it, preventing path-traversal
//! out of the workspace.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput};

// ── Path validation helper ─────────────────────────────────────────────────────

/// Resolve `path_str` within an optional `workspace_root`.
///
/// When a workspace root is provided:
/// - Absolute paths are rejected.
/// - The canonicalized resolved path must remain under the (canonicalized)
///   workspace root.
/// When no workspace root is provided the path is used as-is.
fn resolve_path(workspace_root: Option<&Path>, path_str: &str) -> Result<PathBuf, AgentError> {
    let path = PathBuf::from(path_str);
    if let Some(root) = workspace_root {
        if path.is_absolute() {
            return Err(AgentError::ToolExecution(format!(
                "absolute path '{path_str}' is not allowed when a workspace root is configured; \
                 use a path relative to the workspace root"
            )));
        }
        let resolved = root.join(&path);
        // For existing paths: canonicalize and verify prefix.
        // For not-yet-existing paths (e.g. write_file to a new location):
        // canonicalize the deepest existing ancestor and check there.
        let (canonical_resolved, canonical_root) = match (resolved.canonicalize(), root.canonicalize()) {
            (Ok(r), Ok(cr)) => (r, cr),
            (Err(_), Ok(cr)) => {
                // Path doesn't exist yet — walk up to the deepest existing ancestor.
                let mut ancestor = resolved.clone();
                loop {
                    if let Some(p) = ancestor.parent() {
                        ancestor = p.to_path_buf();
                        if let Ok(c) = ancestor.canonicalize() {
                            break (c.join(resolved.strip_prefix(&ancestor).unwrap_or(&resolved)), cr);
                        }
                    } else {
                        // Cannot canonicalize at all — fall back to string prefix check.
                        break (resolved.clone(), cr);
                    }
                }
            }
            (_, Err(e)) => {
                return Err(AgentError::ToolExecution(format!(
                    "failed to canonicalize workspace root: {e}"
                )));
            }
        };
        if !canonical_resolved.starts_with(&canonical_root) {
            return Err(AgentError::ToolExecution(format!(
                "path '{path_str}' escapes the workspace root"
            )));
        }
        Ok(root.join(&path))
    } else {
        Ok(path)
    }
}

// ── ReadFileTool ──────────────────────────────────────────────────────────────

/// Read the contents of a file, optionally limiting to a line range.
///
/// # JSON schema
///
/// ```json
/// {
///   "path": "relative/or/absolute/path",
///   "start_line": 1,      // optional, 1-based, inclusive
///   "end_line": 100       // optional, 1-based, inclusive
/// }
/// ```
///
/// Lines beyond 1 000 are truncated and a `truncated: true` flag is included
/// in the response.
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
        "Read the contents of a file.  Optionally restrict to a range of lines \
         (1-based, inclusive).  Returns at most 1 000 lines; use start_line / \
         end_line to paginate through larger files."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read."
                },
                "start_line": {
                    "type": "integer",
                    "description": "First line to return (1-based, inclusive).",
                    "minimum": 1
                },
                "end_line": {
                    "type": "integer",
                    "description": "Last line to return (1-based, inclusive).",
                    "minimum": 1
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let path_str = arguments
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::ToolExecution("missing 'path' argument".into()))?;

        let resolved = resolve_path(self.workspace_root.as_deref(), path_str)?;

        let start_line = arguments.get("start_line").and_then(Value::as_u64).unwrap_or(1) as usize;
        let end_line = arguments.get("end_line").and_then(Value::as_u64).map(|v| v as usize);

        let raw = tokio::fs::read_to_string(&resolved)
            .await
            .map_err(|e| AgentError::ToolExecution(format!("failed to read {path_str}: {e}")))?;

        const MAX_LINES: usize = 1000;

        let start_idx = start_line.saturating_sub(1);
        let all_lines: Vec<&str> = raw.lines().collect();
        let total = all_lines.len();

        // The requested end (exclusive upper bound in Vec indexing).
        let requested_end = end_line.map(|e| e.min(total)).unwrap_or(total);
        // Cap by MAX_LINES relative to the start.
        let capped_end = requested_end.min(start_idx + MAX_LINES);

        let lines: Vec<&str> = all_lines
            .get(start_idx..capped_end)
            .unwrap_or(&[])
            .to_vec();

        // truncated is true when the MAX_LINES cap reduced the response,
        // i.e. there are more lines available beyond what was returned.
        let truncated = capped_end < requested_end;

        Ok(ToolOutput {
            content: serde_json::json!({
                "content": lines.join("\n"),
                "total_lines": total,
                "returned_lines": lines.len(),
                "truncated": truncated
            })
            .to_string(),
            metadata: None,
        })
    }
}

// ── WriteFileTool ─────────────────────────────────────────────────────────────

/// Atomically write content to a file (creates parent directories as needed).
///
/// # JSON schema
///
/// ```json
/// {
///   "path": "relative/or/absolute/path",
///   "content": "file content here"
/// }
/// ```
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
        "Write content to a file.  Creates missing parent directories.  The \
         write is atomic: content is written to a sibling temp file and renamed \
         into place so readers never see a partial write."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Destination file path."
                },
                "content": {
                    "type": "string",
                    "description": "Content to write."
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let path_str = arguments
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::ToolExecution("missing 'path' argument".into()))?;
        let content = arguments
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::ToolExecution("missing 'content' argument".into()))?;

        let path = resolve_path(self.workspace_root.as_deref(), path_str)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                AgentError::ToolExecution(format!("failed to create directories for {path_str}: {e}"))
            })?;
        }

        // Atomic write: write to a tmp path, then rename.
        let tmp_path = {
            let mut name = path.file_name().unwrap_or_default().to_owned();
            name.push(".tmp");
            path.with_file_name(name)
        };

        tokio::fs::write(&tmp_path, content).await.map_err(|e| {
            AgentError::ToolExecution(format!("failed to write temp file: {e}"))
        })?;

        tokio::fs::rename(&tmp_path, &path).await.map_err(|e| {
            AgentError::ToolExecution(format!("failed to rename temp file to {path_str}: {e}"))
        })?;

        Ok(ToolOutput {
            content: serde_json::json!({
                "written": path_str,
                "bytes": content.len()
            })
            .to_string(),
            metadata: None,
        })
    }
}

// ── ListDirTool ───────────────────────────────────────────────────────────────

/// List the immediate children of a directory.
///
/// # JSON schema
///
/// ```json
/// {
///   "path": "relative/or/absolute/directory"
/// }
/// ```
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
        "List the immediate contents of a directory, returning each entry's \
         name, whether it is a directory, its size in bytes, and its last \
         modification time (Unix timestamp)."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory to list."
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let path_str = arguments
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::ToolExecution("missing 'path' argument".into()))?;

        let resolved = resolve_path(self.workspace_root.as_deref(), path_str)?;

        let mut read_dir = tokio::fs::read_dir(&resolved)
            .await
            .map_err(|e| AgentError::ToolExecution(format!("failed to read dir {path_str}: {e}")))?;

        let mut entries = Vec::new();
        while let Some(entry) = read_dir.next_entry().await.map_err(|e| {
            AgentError::ToolExecution(format!("failed to read dir entry: {e}"))
        })? {
            let name = entry.file_name().to_string_lossy().into_owned();
            let meta = entry.metadata().await.ok();
            let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size_bytes = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            entries.push(serde_json::json!({
                "name": name,
                "is_dir": is_dir,
                "size_bytes": size_bytes,
                "modified": modified
            }));
        }

        // Sort: directories first, then alphabetically.
        entries.sort_by(|a, b| {
            let a_dir = a["is_dir"].as_bool().unwrap_or(false);
            let b_dir = b["is_dir"].as_bool().unwrap_or(false);
            b_dir.cmp(&a_dir).then_with(|| {
                a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or(""))
            })
        });

        Ok(ToolOutput {
            content: serde_json::json!({ "entries": entries }).to_string(),
            metadata: None,
        })
    }
}

