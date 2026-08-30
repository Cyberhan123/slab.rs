//! File-system read/write/list tools.

use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{
    AgentError, ToolCallRender, ToolContext, ToolHandler, ToolOutput, protocol::TurnItem,
};
use slab_utils::string::truncate_middle_bytes;

use crate::args::string_arg;

const MAX_LINES: usize = 1000;
/// Byte cap on the returned content — the line cap alone lets files with very
/// long lines (minified/generated) inject unbounded payloads.
const MAX_CONTENT_BYTES: usize = 48 * 1024;
/// Head fraction of the kept budget when the byte cap fires.
const CONTENT_HEAD_RATIO: f32 = 0.7;

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

    /// Pure read — safe to run concurrently with other read-only calls.
    fn is_concurrency_safe(&self, _arguments: &serde_json::Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Read a UTF-8 text file byte-faithfully (line endings and trailing \
         newline preserved), optionally restricted to a 1-based inclusive line \
         range. Returns at most 1000 lines per call (head kept, an omission \
         marker inserted when more lines remain — page with \
         start_line/end_line); content is additionally bounded to 48KB \
         (head+tail kept, middle omitted with a marker) and the full selected \
         range is spilled to an artifact. start_line past the end of the file \
         is an error, not an empty result."
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

    fn describe_operation(&self, arguments: &Value) -> Option<slab_agent::OperationDescriptor> {
        let path = arguments.get("path").and_then(Value::as_str)?;
        Some(
            slab_agent::OperationDescriptor::read_only(path)
                .with_workspace(self.workspace_root.clone()),
        )
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let path = string_arg(arguments, "path")?;
        let start_line = arguments.get("start_line").and_then(Value::as_u64).unwrap_or(1) as usize;
        let end_line = arguments.get("end_line").and_then(Value::as_u64).map(|v| v as usize);
        let path = resolve_agent_path(self.workspace_root.as_deref(), &self.extra_roots, path)?;
        // Read raw BYTES: content must be byte-faithful. Normalizing through
        // `read_to_string` + `lines()` used to strip every `\r` (a CRLF file
        // re-read short) and report the NORMALIZED length as `total_bytes`
        // instead of the on-disk size. `split_inclusive('\n')` keeps each
        // line's terminator (and the `\r` before it) verbatim, including a
        // trailing newline.
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|error| crate::error::io_tool_error("read file", &path, &error))?;
        let total_bytes = bytes.len();
        let raw = String::from_utf8_lossy(&bytes);
        let lines: Vec<&str> = raw.split_inclusive('\n').collect();
        let total = lines.len();

        let start_idx = start_line.saturating_sub(1);
        if start_idx >= total {
            // Paging past EOF is a client error, not an empty result: the old
            // silent-empty behavior read as "file is empty" and misdirected
            // the model.
            return Err(AgentError::ToolExecution(format!(
                "read_file: start_line {start_line} is past the end of the file ({total} lines); \
                 request an existing line range"
            )));
        }

        let requested_end = end_line.map(|end| end.min(total)).unwrap_or(total);
        let capped_end = requested_end.min(start_idx + MAX_LINES);
        let line_cap_truncated = capped_end < requested_end;
        let selected = lines.get(start_idx..capped_end).unwrap_or(&[]);

        let selected_content = selected.concat();
        let mut joined = selected_content.clone();
        if line_cap_truncated {
            // Match the byte-cap contract: an explicit omission signal with a
            // narrowing hint, never a silent head-only cut.
            joined.push_str(&format!(
                "\n[... {} of {total} lines omitted — narrow the range with start_line/end_line ...]\n",
                total - capped_end
            ));
        }
        let (bounded, omitted_bytes) =
            truncate_middle_bytes(&joined, MAX_CONTENT_BYTES, CONTENT_HEAD_RATIO);

        // Spill the full selected content when the byte cap fired — the model
        // can read the artifact instead of paging through line ranges.
        let full_content_artifact = if omitted_bytes > 0 {
            use std::hash::{DefaultHasher, Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            path.to_string_lossy().hash(&mut hasher);
            let nonce = format!("{:016x}", hasher.finish());
            crate::artifact::write_tool_artifact(
                ctx.workspace.as_ref().map(|workspace| workspace.root.as_path()),
                &ctx.thread_id,
                &format!("read-{nonce}-t{}.txt", ctx.turn_index),
                selected_content.as_bytes(),
            )
            .await
        } else {
            None
        };

        let mut envelope = serde_json::json!({
            "content": bounded,
            "total_lines": total,
            "returned_lines": selected.len(),
            "total_bytes": total_bytes,
            "omitted_bytes": omitted_bytes,
            "truncated": line_cap_truncated || omitted_bytes > 0
        });
        if let Some(reference) = full_content_artifact {
            envelope["full_content_artifact"] = serde_json::json!(reference);
        }

        Ok(ToolOutput { content: envelope.to_string(), metadata: None })
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

    fn describe_operation(&self, arguments: &Value) -> Option<slab_agent::OperationDescriptor> {
        let path = arguments.get("path").and_then(Value::as_str)?;
        Some(
            slab_agent::OperationDescriptor::file_edit(path)
                .with_workspace(self.workspace_root.clone()),
        )
    }

    fn category(&self) -> slab_agent::OperationCategory {
        slab_agent::OperationCategory::FileEdit
    }

    fn render_turn_item(&self, render: &ToolCallRender<'_>) -> TurnItem {
        // The diff preview is the incoming content as an all-added block. The
        // render path is sync and must not touch the filesystem, so add-vs-edit
        // cannot be distinguished here — `type` stays "edit" and the UI shows
        // what is about to be written.
        let content = render.args.get("content").and_then(Value::as_str).unwrap_or("");
        let diff = content.lines().map(|line| format!("+{line}")).collect::<Vec<_>>().join("\n");
        TurnItem::FileChange {
            id: render.call.id.clone(),
            changes: vec![serde_json::json!({
                "path": render.args.get("path").and_then(Value::as_str).unwrap_or(""),
                "type": "edit",
                "diff": diff,
            })],
            status: render.status.to_owned(),
        }
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
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                crate::error::io_tool_error("create parent directory", parent, &error)
            })?;
        }
        tokio::fs::write(&path, content)
            .await
            .map_err(|error| crate::error::io_tool_error("write file", &path, &error))?;

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

    /// Pure read — safe to run concurrently with other read-only calls.
    fn is_concurrency_safe(&self, _arguments: &serde_json::Value) -> bool {
        true
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

    fn describe_operation(&self, arguments: &Value) -> Option<slab_agent::OperationDescriptor> {
        let path = arguments.get("path").and_then(Value::as_str)?;
        Some(
            slab_agent::OperationDescriptor::read_only(path)
                .with_workspace(self.workspace_root.clone()),
        )
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
        // Byte-faithful: the selected lines keep their terminators, including
        // the file's trailing newline.
        assert_eq!(value["content"], "two\nthree\n");
        assert_eq!(value["total_lines"], 3);
        assert_eq!(value["returned_lines"], 2);
        assert_eq!(value["truncated"], false);

        // Out-of-range start is a client error, not a silent empty read (the
        // old empty-string behavior read as "file is empty").
        let error = tool
            .execute(&ctx(), &json!({"path": "notes.txt", "start_line": 2_000}))
            .await
            .expect_err("out of range read");
        assert!(error.to_string().contains("past the end of the file"));

        let _ = fs::remove_dir_all(root);
    }

    /// Byte fidelity: CRLF line endings and the trailing newline survive a
    /// round trip; `total_bytes` reports the on-disk size, not the normalized
    /// length.
    #[tokio::test]
    async fn read_file_preserves_crlf_and_trailing_newline() {
        let root = temp_root("read_crlf");
        fs::write(root.join("crlf.txt"), b"AB\r\nCD\n").expect("seed file");
        let tool = ReadFileTool::new(Some(root.clone()));

        let output = tool.execute(&ctx(), &json!({"path": "crlf.txt"})).await.expect("read file");
        let value: Value = serde_json::from_str(&output.content).expect("json output");
        assert_eq!(value["content"], "AB\r\nCD\n");
        assert_eq!(value["total_bytes"], 7);
        assert_eq!(value["total_lines"], 2);

        let _ = fs::remove_dir_all(root);
    }

    /// The 1000-line cap leaves an explicit omission marker with a narrowing
    /// hint instead of a silent head-only cut.
    #[tokio::test]
    async fn read_file_line_cap_inserts_omission_marker() {
        let root = temp_root("read_linecap");
        let content = (1..=1_500).map(|idx| format!("l{idx:04}")).collect::<Vec<_>>().join("\n");
        fs::write(root.join("long.txt"), &content).expect("seed file");
        let tool = ReadFileTool::new(Some(root.clone()));

        let output = tool.execute(&ctx(), &json!({"path": "long.txt"})).await.expect("read file");
        let value: Value = serde_json::from_str(&output.content).expect("json output");
        let returned = value["content"].as_str().expect("content");
        assert_eq!(value["returned_lines"], 1000);
        assert_eq!(value["truncated"], true);
        assert!(returned.contains("lines omitted"), "marker missing: {returned}");
        assert!(returned.contains("l1000\n"), "head must reach the cap");
        assert!(!returned.contains("\nl1001\n"), "lines past the cap must be omitted");

        let _ = fs::remove_dir_all(root);
    }

    /// P5 regression: a missing file must report a coded English error, not
    /// the OS-localized io message ("系统找不到指定的文件。 (os error 2)").
    #[tokio::test]
    async fn read_file_reports_coded_error_for_missing_file() {
        let root = temp_root("missing_read");
        let tool = ReadFileTool::new(Some(root.clone()));

        let error = tool
            .execute(&ctx(), &json!({"path": "does_not_exist.txt"}))
            .await
            .expect_err("missing file");
        let rendered = error.to_string();
        assert!(rendered.contains("[io.not_found]"), "{rendered}");
        assert!(rendered.contains("not found"), "{rendered}");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn read_and_list_tools_describe_read_only_operations() {
        let read = ReadFileTool::new(Some(PathBuf::from(".")));
        let list = ListDirTool::new(Some(PathBuf::from(".")));

        let read_desc =
            read.describe_operation(&json!({"path": "src/main.rs"})).expect("descriptor");
        assert_eq!(read_desc.category, slab_agent::OperationCategory::ReadOnly);
        assert_eq!(read_desc.subject, "src/main.rs");

        let list_desc = list.describe_operation(&json!({"path": "src"})).expect("descriptor");
        assert_eq!(list_desc.category, slab_agent::OperationCategory::ReadOnly);
        assert_eq!(list_desc.subject, "src");

        // Sensitive-path protection now lives in the exec-policy engine, not
        // the tool — `describe_operation` returns a descriptor regardless.
        assert!(read.describe_operation(&json!({"path": ".env"})).is_some());
    }

    #[test]
    fn write_file_renders_file_change_with_added_lines_diff() {
        let tool = WriteFileTool::new(Some(PathBuf::from(".")));
        let call = slab_agent::port::ParsedToolCall {
            id: "c1".into(),
            name: "write_file".into(),
            arguments: "{}".into(),
        };
        let args = json!({ "path": "notes/a.txt", "content": "alpha\nbeta" });
        let render = ToolCallRender {
            call: &call,
            args: &args,
            status: "running",
            output: None,
            workspace_root: None,
            exit_code: None,
            duration_ms: None,
        };
        match tool.render_turn_item(&render) {
            TurnItem::FileChange { changes, status, .. } => {
                assert_eq!(status, "running");
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0]["path"].as_str(), Some("notes/a.txt"));
                assert_eq!(changes[0]["type"].as_str(), Some("edit"));
                assert_eq!(changes[0]["diff"].as_str(), Some("+alpha\n+beta"));
            }
            other => panic!("unexpected item: {other:?}"),
        }
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

    /// The line cap alone lets long-lined files inject unbounded payloads;
    /// the byte cap keeps head and tail with an explicit marker.
    #[tokio::test]
    async fn read_file_tool_caps_content_bytes_with_marker() {
        let root = temp_root("read_bytes");
        let line = format!("line-{}-", "z".repeat(194));
        let content =
            (0..1_000).map(|idx| format!("{line}{idx:04}")).collect::<Vec<_>>().join("\n");
        fs::write(root.join("wide.txt"), &content).expect("seed file");
        let tool = ReadFileTool::new(Some(root.clone()));

        let output = tool.execute(&ctx(), &json!({"path": "wide.txt"})).await.expect("read file");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        let returned = value["content"].as_str().expect("content");
        assert!(
            returned.len() < MAX_CONTENT_BYTES + 128,
            "content not bounded: {}",
            returned.len()
        );
        assert!(returned.starts_with("line-"), "head must survive");
        assert!(returned.contains("0999"), "tail must survive");
        assert!(returned.contains("bytes omitted"), "marker missing");
        assert!(value["omitted_bytes"].as_u64().expect("omitted") > 0);
        assert_eq!(value["total_bytes"].as_u64().expect("total") as usize, content.len());
        assert_eq!(value["truncated"], true);

        let _ = fs::remove_dir_all(root);
    }

    /// P2 regression: write_file must never echo the written content back —
    /// the model just wrote it and does not need it re-injected.
    #[tokio::test]
    async fn write_file_tool_does_not_echo_content() {
        let root = temp_root("write_no_echo");
        let payload = "SECRETPAYLOAD-".repeat(400); // ~6KB
        let tool = WriteFileTool::new(Some(root.clone()));

        let output = tool
            .execute(&ctx(), &json!({"path": "out.txt", "content": payload}))
            .await
            .expect("write file");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert_eq!(value["written"], "out.txt");
        assert_eq!(value["bytes"], payload.len());
        assert!(value.get("content").is_none(), "content key must not exist");
        assert!(!output.content.contains("SECRETPAYLOAD"), "payload echoed back");

        let _ = fs::remove_dir_all(root);
    }

    /// When the byte cap fires, the full selected content spills to a
    /// workspace artifact referenced from the envelope.
    #[tokio::test]
    async fn read_file_tool_spills_full_content_when_capped() {
        let root = temp_root("read_spill");
        let line = format!("line-{}-", "z".repeat(194));
        let content =
            (0..1_000).map(|idx| format!("{line}{idx:04}")).collect::<Vec<_>>().join("\n");
        fs::write(root.join("wide.txt"), &content).expect("seed file");
        let tool = ReadFileTool::new(Some(root.clone()));
        let ctx = ToolContext::for_thread("spill-read-thread")
            .workspace(slab_agent::WorkspaceRef { root: root.clone(), session_id: None })
            .build();

        let output = tool.execute(&ctx, &json!({"path": "wide.txt"})).await.expect("read file");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert!(value["omitted_bytes"].as_u64().expect("omitted") > 0);
        let reference = value["full_content_artifact"].as_str().expect("artifact reference");
        assert!(reference.starts_with(".slab/artifacts/spill-read-thread/read-"));
        assert!(reference.ends_with("-t0.txt"));
        let spilled = fs::read_to_string(root.join(reference)).expect("artifact exists");
        assert_eq!(spilled.len(), content.len(), "artifact holds the full content");

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
