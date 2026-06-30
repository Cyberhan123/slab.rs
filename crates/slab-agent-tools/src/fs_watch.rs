//! File-system watcher tool backed by `slab-file`.
//!
//! Wraps the `FileWatcher` from the local slab crate to provide a
//! one-shot "wait for file changes" tool call.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput};
use slab_file::watcher::{FileWatcher, WatchPath};

/// Watch a path for file-system changes and return the list of changed paths.
///
/// The tool subscribes to the watcher, waits up to `timeout_ms` milliseconds
/// for the first batch of events, then returns.
///
/// # JSON schema
///
/// ```json
/// {
///   "path": "/absolute/or/relative/path",
///   "recursive": true,         // default true
///   "timeout_ms": 2000         // default 2000
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
impl ToolHandler for FsWatchTool {
    fn name(&self) -> &str {
        "fs_watch"
    }

    fn description(&self) -> &str {
        "Watch a file-system path for changes.  Returns the list of changed \
         paths after the first change event arrives, or an empty list if the \
         timeout is reached."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to watch."
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Watch subdirectories recursively.",
                    "default": true
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "How long to wait for an event (milliseconds).",
                    "default": 2000
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

        let recursive = arguments.get("recursive").and_then(Value::as_bool).unwrap_or(true);
        let timeout_ms = arguments.get("timeout_ms").and_then(Value::as_u64).unwrap_or(2000);

        let watch_path = WatchPath { path: PathBuf::from(path_str), recursive };

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
        let schema = FsWatchTool::noop().parameters_schema();

        assert_eq!(schema["properties"]["path"]["type"], "string");
        assert_eq!(schema["properties"]["recursive"]["default"], true);
        assert_eq!(schema["properties"]["timeout_ms"]["default"], 2000);
        assert_eq!(schema["required"], json!(["path"]));
    }

    #[tokio::test]
    async fn fs_watch_requires_path_argument() {
        let tool = FsWatchTool::noop();

        let error = tool.execute(&ctx(), &json!({})).await.expect_err("missing path");

        assert_eq!(error.to_string(), "tool execution error: missing 'path' argument");
    }

    #[tokio::test]
    async fn noop_fs_watch_times_out_with_empty_change_list() {
        let tool = FsWatchTool::noop();

        let output = tool
            .execute(&ctx(), &json!({"path": ".", "recursive": false, "timeout_ms": 1}))
            .await
            .expect("watch output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert_eq!(value["changed_paths"], json!([]));
        assert_eq!(value["timed_out"], true);
    }
}
