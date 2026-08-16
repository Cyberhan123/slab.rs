//! `*** Begin Patch` application tool.
//!
//! Drives the [`slab_apply_patch`] engine (the OpenAI/Codex `*** Begin Patch`
//! dialect with fuzzy context matching and partial-failure delta tracking)
//! against the configured workspace root through slab's `ExecutorFileSystem`.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{
    AgentError, ToolCallRender, ToolContext, ToolHandler, ToolOutput, ToolOutputObserver,
    ToolOutputStream, protocol::TurnItem,
};
use slab_apply_patch::{
    AppliedPatchDelta, AppliedPatchFileChange, Hunk, PatchProgress, PatchProgressKind,
    PatchProgressSink, UpdateFileChunk, apply_patch_with_progress as apply_patch_engine,
    local_file_system, parse_patch,
};
use slab_file::{FileSystemSandboxContext, FileSystemSandboxPolicy};
use slab_utils::path::absolute::AbsolutePathBuf;

pub struct ApplyPatchTool {
    workspace_root: PathBuf,
}

impl ApplyPatchTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl ToolHandler for ApplyPatchTool {
    fn name(&self) -> &str {
        "apply_patch"
    }

    fn description(&self) -> &str {
        "Apply a `*** Begin Patch` / `*** End Patch` patch to files inside the \
         configured workspace root. Wrap the patch with `*** Begin Patch` and \
         `*** End Patch`. Use `*** Add File: <path>` followed by `+<line>` \
         lines to create a file, `*** Delete File: <path>` to remove one, or \
         `*** Update File: <path>` followed by one or more `@@` chunks (each \
         context line prefixed with a single space, removed lines with `-`, \
         added lines with `+`; optional `*** Move to: <path>` to rename and \
         `*** End of File` to anchor the end of the file). Updates match the \
         surrounding context leniently, and a partial application reports \
         which files already changed."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "patch": {
                    "type": "string",
                    "description": "Patch text in the `*** Begin Patch` dialect (optionally wrapped in an `apply_patch <<'EOF' … EOF` heredoc)."
                }
            },
            "required": ["patch"]
        })
    }

    fn describe_operation(&self, arguments: &Value) -> Option<slab_agent::OperationDescriptor> {
        let patch = arguments.get("patch").and_then(Value::as_str)?;
        Some(
            slab_agent::OperationDescriptor::file_edit(first_path_in_patch(patch))
                .with_workspace(Some(self.workspace_root.clone()))
                .with_detail(patch),
        )
    }

    fn category(&self) -> slab_agent::OperationCategory {
        slab_agent::OperationCategory::FileEdit
    }

    fn render_turn_item(&self, render: &ToolCallRender<'_>) -> TurnItem {
        let patch = render.args.get("patch").and_then(Value::as_str).unwrap_or("");
        TurnItem::FileChange {
            id: render.call.id.clone(),
            changes: patch_changes(patch),
            status: render.status.to_owned(),
        }
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let patch = arguments
            .get("patch")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::ToolExecution("missing 'patch' argument".into()))?;

        // `workspace_root` may be relative (e.g. registration tests pass "."),
        // so absolutize it infallibly against the process cwd. `cwd` and the
        // sandbox `workspace_root` MUST be the same absolute path: the engine
        // strips `cwd` to form a relative path string and the local filesystem
        // adapter re-anchors it via `resolve_path(workspace_root, …)`.
        let base = std::env::current_dir()
            .map_err(|error| AgentError::ToolExecution(error.to_string()))?;
        let cwd = AbsolutePathBuf::resolve_path_against_base(&self.workspace_root, &base);
        let root = cwd.as_path().to_path_buf();
        let sandbox = FileSystemSandboxContext {
            policy: FileSystemSandboxPolicy::WorkspaceWrite,
            cwd: Some(root.clone()),
            workspace_root: Some(root),
            readable_roots: Vec::new(),
            writable_roots: Vec::new(),
            denied_paths: Vec::new(),
        };

        // The engine writes a human-readable "Success." summary to stdout on
        // full success and diagnostics to stderr on failure; the structured
        // result is derived from the returned delta, not these sinks. Each
        // committed file is reported through `ctx.output` so the host can show
        // live "file applied" progress while the patch runs.
        let progress = ToolProgressSink { observer: ctx.output.clone() };
        let mut stdout_sink: Vec<u8> = Vec::new();
        let mut stderr_sink: Vec<u8> = Vec::new();
        let outcome = apply_patch_engine(
            patch,
            &cwd,
            &mut stdout_sink,
            &mut stderr_sink,
            local_file_system(),
            Some(&sandbox),
            Some(&progress),
        )
        .await;

        let content = match outcome {
            Ok(delta) => render_ok(&delta),
            Err(failure) => {
                let (error, delta) = failure.into_parts();
                render_err(&error.to_string(), &delta)
            }
        };

        Ok(ToolOutput { content, metadata: None })
    }
}

/// Forwards each committed file from the apply engine to the host tool-output
/// observer as a JSON line `{"path": ..., "kind": "add"|"modify"|"delete"}`,
/// which the agent kernel routes to `FileChangeOutputDelta` for live UI progress.
struct ToolProgressSink {
    observer: Option<Arc<dyn ToolOutputObserver>>,
}

impl PatchProgressSink for ToolProgressSink {
    fn on_progress(&self, progress: PatchProgress<'_>) {
        let Some(observer) = self.observer.as_ref() else {
            return;
        };
        let kind = match progress.kind {
            PatchProgressKind::Add => "add",
            PatchProgressKind::Modify => "modify",
            PatchProgressKind::Delete => "delete",
        };
        let line = serde_json::json!({
            "path": progress.path.display().to_string(),
            "kind": kind,
        })
        .to_string();
        observer.on_output(ToolOutputStream::Stdout, &line);
    }
}

/// Split a committed [`AppliedPatchDelta`] into `(added, modified, deleted)`
/// path lists, preserving application order.
fn classify(delta: &AppliedPatchDelta) -> (Vec<String>, Vec<String>, Vec<String>) {
    let (mut added, mut modified, mut deleted) = (Vec::new(), Vec::new(), Vec::new());
    for change in delta.changes() {
        let path = change.path.to_string_lossy().into_owned();
        match change.change {
            AppliedPatchFileChange::Add { .. } => added.push(path),
            AppliedPatchFileChange::Delete { .. } => deleted.push(path),
            AppliedPatchFileChange::Update { .. } => modified.push(path),
        }
    }
    (added, modified, deleted)
}

/// Render a successful application as the tool-result JSON. Keeps the legacy
/// `applied_files` / `result` fields and adds per-kind lists plus the engine's
/// `exact` flag.
fn render_ok(delta: &AppliedPatchDelta) -> String {
    let (added, modified, deleted) = classify(delta);
    let mut applied_files = added.clone();
    applied_files.extend(modified.iter().cloned());
    applied_files.extend(deleted.iter().cloned());
    serde_json::json!({
        "result": "ok",
        "added": added,
        "modified": modified,
        "deleted": deleted,
        "applied_files": applied_files,
        "exact": delta.is_exact(),
    })
    .to_string()
}

/// Render a failed application, including the files committed before the
/// failure was observed (may be non-empty) so the model can reason about
/// partial state instead of assuming nothing landed.
fn render_err(message: &str, delta: &AppliedPatchDelta) -> String {
    let (added, modified, deleted) = classify(delta);
    let mut applied_files = added.clone();
    applied_files.extend(modified.iter().cloned());
    applied_files.extend(deleted.iter().cloned());
    serde_json::json!({
        "result": "error",
        "error_message": message,
        "added": added,
        "modified": modified,
        "deleted": deleted,
        "applied_files": applied_files,
        "exact": delta.is_exact(),
    })
    .to_string()
}

/// Extract the first modified file path from a patch, for the file-edit
/// descriptor subject. Recognizes the `*** Begin Patch` dialect headers and
/// keeps legacy unified-diff `+++ b/path` support as a fallback. Returns
/// `"patch"` when no path can be parsed.
fn first_path_in_patch(patch: &str) -> String {
    for line in patch.lines() {
        let trimmed = line.trim_start();
        for header in ["*** Add File:", "*** Delete File:", "*** Update File:"] {
            if let Some(rest) = trimmed.strip_prefix(header) {
                let path = rest.trim().trim_matches('"');
                if !path.is_empty() {
                    return path.to_owned();
                }
            }
        }
        if let Some(rest) = trimmed.strip_prefix("+++ ") {
            let candidate = rest.trim();
            let path = candidate.strip_prefix("b/").unwrap_or(candidate).trim_matches('"');
            if !path.is_empty() && path != "/dev/null" {
                return path.to_owned();
            }
        }
    }
    "patch".to_owned()
}

/// Build the per-file change list for the `FileChange` turn item (and the
/// file-change approval banner derived from it). Parses the patch into hunks
/// so multi-file patches surface every file with its kind; falls back to the
/// legacy single-entry form (first path + the whole patch text) when the
/// patch is not parseable (e.g. unified diffs or heredoc wrappers) — the same
/// parse the engine itself applies, so the card never disagrees with the
/// execution outcome.
fn patch_changes(patch: &str) -> Vec<Value> {
    let hunks = match parse_patch(patch) {
        Ok(args) if !args.hunks.is_empty() => args.hunks,
        _ => {
            return vec![serde_json::json!({
                "path": first_path_in_patch(patch),
                "type": "edit",
                "diff": patch,
            })];
        }
    };
    hunks
        .iter()
        .map(|hunk| match hunk {
            Hunk::AddFile { path, contents } => serde_json::json!({
                "path": path.to_string_lossy(),
                "type": "add",
                "diff": contents
                    .lines()
                    .map(|line| format!("+{line}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            }),
            Hunk::DeleteFile { path } => serde_json::json!({
                "path": path.to_string_lossy(),
                "type": "delete",
            }),
            Hunk::UpdateFile { path, move_path, chunks } => serde_json::json!({
                "path": move_path.as_ref().unwrap_or(path).to_string_lossy(),
                "type": "edit",
                "diff": update_chunks_diff(chunks),
            }),
        })
        .collect()
}

/// Synthesize a display-only diff from update chunks. The apply-patch dialect
/// stores `old_lines`/`new_lines` without unchanged context, so each chunk
/// renders as its `@@` context header followed by the removed/added lines.
/// Sync and filesystem-free (the render path must not touch the FS); this is
/// for the preview card, not for application.
fn update_chunks_diff(chunks: &[UpdateFileChunk]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for chunk in chunks {
        match chunk.change_context.as_deref() {
            Some(context) if !context.is_empty() => lines.push(format!("@@ {context}")),
            _ => lines.push("@@".to_owned()),
        }
        for line in &chunk.old_lines {
            lines.push(format!("-{line}"));
        }
        for line in &chunk.new_lines {
            lines.push(format!("+{line}"));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::{Value, json};
    use slab_agent::{ToolContext, ToolHandler};

    use super::*;

    fn ctx() -> ToolContext {
        ToolContext::for_thread("thread").build()
    }

    #[test]
    fn apply_patch_renders_file_change_with_first_path() {
        let tool = ApplyPatchTool::new(PathBuf::from("."));
        let patch = "--- a/x.rs\n+++ b/x.rs\n@@ -1 +1 @@\n-a\n+b\n";
        let call = slab_agent::port::ParsedToolCall {
            id: "c1".into(),
            name: "apply_patch".into(),
            arguments: "{}".into(),
        };
        let args = json!({ "patch": patch });
        let render = ToolCallRender {
            call: &call,
            args: &args,
            status: "completed",
            output: None,
            workspace_root: None,
            exit_code: None,
            duration_ms: None,
        };
        match tool.render_turn_item(&render) {
            TurnItem::FileChange { changes, status, .. } => {
                assert_eq!(status, "completed");
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0]["path"].as_str(), Some("x.rs"));
                assert_eq!(changes[0]["type"].as_str(), Some("edit"));
                assert_eq!(changes[0]["diff"].as_str(), Some(patch));
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn apply_patch_renders_per_file_changes_from_begin_patch_dialect() {
        let tool = ApplyPatchTool::new(PathBuf::from("."));
        let patch = concat!(
            "*** Begin Patch\n",
            "*** Add File: new.txt\n",
            "+hello\n",
            "+world\n",
            "*** Update File: mod.rs\n",
            "@@ fn main\n",
            "-old\n",
            "+new\n",
            "*** Delete File: gone.txt\n",
            "*** End Patch\n",
        );
        let call = slab_agent::port::ParsedToolCall {
            id: "c1".into(),
            name: "apply_patch".into(),
            arguments: "{}".into(),
        };
        let args = json!({ "patch": patch });
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
                assert_eq!(changes.len(), 3);
                assert_eq!(changes[0]["path"].as_str(), Some("new.txt"));
                assert_eq!(changes[0]["type"].as_str(), Some("add"));
                assert_eq!(changes[0]["diff"].as_str(), Some("+hello\n+world"));
                assert_eq!(changes[1]["path"].as_str(), Some("mod.rs"));
                assert_eq!(changes[1]["type"].as_str(), Some("edit"));
                assert_eq!(changes[1]["diff"].as_str(), Some("@@ fn main\n-old\n+new"));
                assert_eq!(changes[2]["path"].as_str(), Some("gone.txt"));
                assert_eq!(changes[2]["type"].as_str(), Some("delete"));
                assert!(changes[2].get("diff").is_none());
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn apply_patch_render_falls_back_for_heredoc_wrapped_patch() {
        let tool = ApplyPatchTool::new(PathBuf::from("."));
        let patch = concat!(
            "apply_patch <<'EOF'\n",
            "*** Begin Patch\n",
            "*** Add File: foo\n",
            "+hi\n",
            "*** End Patch\n",
            "EOF",
        );
        let call = slab_agent::port::ParsedToolCall {
            id: "c1".into(),
            name: "apply_patch".into(),
            arguments: "{}".into(),
        };
        let args = json!({ "patch": patch });
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
            TurnItem::FileChange { changes, .. } => {
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0]["path"].as_str(), Some("foo"));
                assert_eq!(changes[0]["type"].as_str(), Some("edit"));
                assert_eq!(changes[0]["diff"].as_str(), Some(patch));
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    fn abs(root: &Path, name: &str) -> String {
        root.join(name).to_string_lossy().into_owned()
    }

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "slab_agent_tools_patch_{name}_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    #[tokio::test]
    async fn apply_patch_tool_applies_begin_patch_update() {
        let root = temp_root("update");
        fs::write(root.join("a.txt"), "one\ntwo\n").expect("seed file");
        let tool = ApplyPatchTool::new(root.clone());
        let patch = "\
*** Begin Patch
*** Update File: a.txt
@@
-two
+three
*** End Patch\n";

        let output = tool.execute(&ctx(), &json!({ "patch": patch })).await.expect("patch output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");
        assert_eq!(value["result"], "ok");
        assert_eq!(value["modified"], json!([abs(&root, "a.txt")]));
        assert_eq!(value["exact"], true);
        assert_eq!(fs::read_to_string(root.join("a.txt")).unwrap(), "one\nthree\n");

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn apply_patch_tool_adds_and_deletes_files() {
        let root = temp_root("add_delete");
        fs::write(root.join("old.txt"), "gone\n").expect("seed file");
        let tool = ApplyPatchTool::new(root.clone());
        let patch = "\
*** Begin Patch
*** Add File: new.txt
+created
*** Delete File: old.txt
*** End Patch\n";

        let output = tool.execute(&ctx(), &json!({ "patch": patch })).await.expect("patch output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");
        assert_eq!(value["result"], "ok");
        assert_eq!(value["added"], json!([abs(&root, "new.txt")]));
        assert_eq!(value["deleted"], json!([abs(&root, "old.txt")]));
        assert_eq!(fs::read_to_string(root.join("new.txt")).unwrap(), "created\n");
        assert!(!root.join("old.txt").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn apply_patch_tool_reports_context_mismatch_as_error() {
        let root = temp_root("mismatch");
        fs::write(root.join("a.txt"), "one\ntwo\n").expect("seed file");
        let tool = ApplyPatchTool::new(root.clone());
        let patch = "\
*** Begin Patch
*** Update File: a.txt
@@
-two
+three
*** End Patch\n";

        // First application succeeds.
        tool.execute(&ctx(), &json!({ "patch": patch })).await.expect("first apply");
        // Second application can no longer find `two` (now `three`).
        let output = tool.execute(&ctx(), &json!({ "patch": patch })).await.expect("patch output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");
        assert_eq!(value["result"], "error");
        assert!(
            value["error_message"]
                .as_str()
                .expect("error message")
                .contains("Failed to find expected lines"),
            "error_message was: {value}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn apply_patch_tool_reports_partial_failure_delta() {
        let root = temp_root("partial");
        let tool = ApplyPatchTool::new(root.clone());
        // Add `created.txt`, then try to update `missing.txt` (does not exist).
        // The add commits before the update fails, mirroring scenario 015.
        let patch = "\
*** Begin Patch
*** Add File: created.txt
+hello
*** Update File: missing.txt
@@
-old
+new
*** End Patch\n";

        let output = tool.execute(&ctx(), &json!({ "patch": patch })).await.expect("patch output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");
        assert_eq!(value["result"], "error");
        assert!(
            value["error_message"]
                .as_str()
                .expect("error message")
                .contains("Failed to read file to update"),
            "error_message was: {value}"
        );
        // The add landed before the failure.
        assert_eq!(value["added"], json!([abs(&root, "created.txt")]));
        assert_eq!(fs::read_to_string(root.join("created.txt")).unwrap(), "hello\n");

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn apply_patch_tool_requires_patch_argument() {
        let root = temp_root("apply_patch_missing");
        let tool = ApplyPatchTool::new(root.clone());

        let error = tool.execute(&ctx(), &json!({})).await.expect_err("missing patch rejected");

        assert_eq!(error.to_string(), "tool execution error: missing 'patch' argument");
        let _ = fs::remove_dir_all(root);
    }
}
