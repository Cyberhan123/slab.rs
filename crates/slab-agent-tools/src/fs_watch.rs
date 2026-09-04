//! File-system watcher tool backed by `slab-file`.
//!
//! Wraps the `FileWatcher` from the local slab crate to provide a
//! one-shot "wait for file changes" tool call.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolOutput, TypedTool};
use slab_file::watcher::{FileWatcher, WatchPath};

/// Upper bound on the blocking wait. The model controls `timeout_ms`; without
/// a cap a single 60s watch serialized its whole tool batch (the tool used to
/// be concurrency-unsafe, so it ran ALONE in a serial batch).
const FS_WATCH_MAX_TIMEOUT_MS: u64 = 30_000;

/// Arguments for the `fs_watch` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FsWatchArgs {
    /// Path to watch (workspace-relative paths resolve against the workspace root).
    path: String,
    /// Watch subdirectories recursively.
    #[serde(default = "default_recursive")]
    recursive: bool,
    /// How long to wait for an event (milliseconds); clamped to at most 30000.
    #[serde(default = "default_timeout_ms")]
    #[schemars(range(max = 30_000))]
    timeout_ms: u64,
}

fn default_recursive() -> bool {
    true
}

fn default_timeout_ms() -> u64 {
    2000
}

/// Watch a path for file-system changes and return the list of changed paths.
///
/// The tool subscribes to the watcher, waits up to `timeout_ms` milliseconds
/// (capped at `FS_WATCH_MAX_TIMEOUT_MS`) for the first batch of events,
/// then returns.
///
/// # JSON schema
///
/// ```json
/// {
///   "path": "/absolute/or/relative/path",
///   "recursive": true,         // default true
///   "timeout_ms": 2000         // default 2000, max 30000
/// }
/// ```
pub struct FsWatchTool {
    watcher: Arc<FileWatcher>,
}

impl FsWatchTool {
    /// Create a live watcher.  Returns `None` if the OS watcher cannot be
    /// initialised (e.g. inotify limit reached).
    pub fn new() -> Option<Self> {
        FileWatcher::new().ok().map(|w| Self { watcher: Arc::new(w) })
    }

    /// Create an inert watcher suitable for tests.
    pub fn noop() -> Self {
        Self { watcher: Arc::new(FileWatcher::noop()) }
    }
}

#[async_trait]
impl TypedTool for FsWatchTool {
    type Input = FsWatchArgs;
    fn name(&self) -> &str {
        "fs_watch"
    }

    fn description(&self) -> &str {
        "Watch a file-system path for changes. BLOCKS until the first change \
         event arrives or timeout_ms elapses (capped at 30s), then returns the \
         list of changed paths (empty + timed_out=true when no event landed). \
         The path resolves against the active workspace when relative. Use \
         this for short waits on a specific file/dir, not as a long-running \
         monitor."
    }

    /// Pure observation — safe to run concurrently with other read-only calls
    /// (and NOT serialized behind write tools, which used to stall a whole
    /// tool batch for the full watch timeout).
    fn is_concurrency_safe(&self, _arguments: &Value) -> bool {
        true
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        args: FsWatchArgs,
    ) -> Result<ToolOutput, AgentError> {
        let recursive = args.recursive;
        let timeout_ms = args.timeout_ms.min(FS_WATCH_MAX_TIMEOUT_MS);

        // Resolve against the active workspace so a relative path does not
        // silently land in the server process's CWD.
        let path = match ctx.workspace.as_ref() {
            Some(workspace) => workspace.root.join(&args.path),
            None => PathBuf::from(&args.path),
        };
        let watch_path = WatchPath { path, recursive };

        let (subscriber, mut rx) = self.watcher.add_subscriber();
        subscriber.register_paths(vec![watch_path]);

        let result = tokio::time::timeout(Duration::from_millis(timeout_ms), rx.recv()).await;

        let (changed_paths, timed_out) = match result {
            Err(_) => (vec![], true),
            Ok(None) => (vec![], false),
            Ok(Some(event)) => {
                let paths: Vec<String> =
                    event.paths.into_iter().filter_map(|p| p.to_str().map(str::to_owned)).collect();
                (paths, false)
            }
        };

        Ok(ToolOutput {
            content: serde_json::json!({
                "changed_paths": changed_paths,
                "timed_out": timed_out
            })
            .to_string(),
            metadata: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use slab_agent::{ToolContext, ToolHandler};

    use super::*;

    fn ctx() -> ToolContext {
        ToolContext::for_thread("thread").build()
    }

    #[test]
    fn fs_watch_schema_requires_path() {
        let schema = ToolHandler::parameters_schema(&FsWatchTool::noop());

        assert_eq!(schema["properties"]["path"]["type"], "string");
        assert_eq!(schema["properties"]["recursive"]["default"], true);
        assert_eq!(schema["properties"]["timeout_ms"]["default"], 2000);
        assert_eq!(schema["required"], json!(["path"]));
    }

    #[tokio::test]
    async fn fs_watch_requires_path_argument() {
        let tool = FsWatchTool::noop();

        let error =
            ToolHandler::execute(&tool, &ctx(), &json!({})).await.expect_err("missing path");

        assert_eq!(error.to_string(), "tool execution error: missing 'path' argument");
    }

    #[tokio::test]
    async fn noop_fs_watch_times_out_with_empty_change_list() {
        let tool = FsWatchTool::noop();

        let output = ToolHandler::execute(
            &tool,
            &ctx(),
            &json!({"path": ".", "recursive": false, "timeout_ms": 1}),
        )
        .await
        .expect("watch output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert_eq!(value["changed_paths"], json!([]));
        assert_eq!(value["timed_out"], true);
    }
}
