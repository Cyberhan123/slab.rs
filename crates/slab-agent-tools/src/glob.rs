//! Gitignore-aware file glob tool.

use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput};

use crate::args::string_arg;

const DEFAULT_MAX_RESULTS: usize = 200;
const HARD_MAX_RESULTS: usize = 1000;

pub struct FileGlobTool {
    workspace_root: Option<PathBuf>,
    extra_roots: Vec<PathBuf>,
}

impl FileGlobTool {
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
impl ToolHandler for FileGlobTool {
    fn name(&self) -> &str {
        "file_glob"
    }

    fn description(&self) -> &str {
        "Find files by gitignore-aware glob pattern inside a workspace path."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match, e.g. '*.rs' or 'src/**/*.ts'."
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search (default: workspace root or '.').",
                    "default": "."
                },
                "max_results": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 1000,
                    "default": 200
                },
                "include_dirs": {
                    "type": "boolean",
                    "description": "Whether matching directories should be included.",
                    "default": false
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let pattern = string_arg(arguments, "pattern")?.to_owned();
        let path = arguments.get("path").and_then(Value::as_str).unwrap_or(".");
        let max_results = arguments
            .get("max_results")
            .and_then(Value::as_u64)
            .map(|value| value.clamp(1, HARD_MAX_RESULTS as u64) as usize)
            .unwrap_or(DEFAULT_MAX_RESULTS);
        let include_dirs = arguments.get("include_dirs").and_then(Value::as_bool).unwrap_or(false);
        let search_root =
            crate::fs::resolve_agent_path(self.workspace_root.as_deref(), &self.extra_roots, path)?;

        let results = tokio::task::spawn_blocking(move || {
            glob_blocking(&search_root, &pattern, max_results, include_dirs)
        })
        .await
        .map_err(|error| AgentError::ToolExecution(format!("file_glob task panicked: {error}")))?;
        let results = results
            .map_err(|error| AgentError::ToolExecution(format!("file_glob failed: {error}")))?;

        Ok(ToolOutput {
            content: serde_json::json!({
                "matches": results,
                "total": results.len(),
                "truncated": results.len() >= max_results
            })
            .to_string(),
            metadata: None,
        })
    }
}

fn glob_blocking(
    root: &std::path::Path,
    pattern: &str,
    max_results: usize,
    include_dirs: bool,
) -> Result<Vec<serde_json::Value>, String> {
    let mut builder = ignore::WalkBuilder::new(root);
    builder.hidden(false);
    builder.require_git(false);

    let mut override_builder = ignore::overrides::OverrideBuilder::new(root);
    override_builder.add(pattern).map_err(|error| format!("invalid glob: {error}"))?;
    let overrides =
        override_builder.build().map_err(|error| format!("glob build error: {error}"))?;
    builder.overrides(overrides);

    let mut results = Vec::new();
    for result in builder.build() {
        if results.len() >= max_results {
            break;
        }
        let entry = match result {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        let is_dir = file_type.is_dir();
        if is_dir && !include_dirs {
            continue;
        }
        results.push(serde_json::json!({
            "path": entry.path().display().to_string(),
            "kind": if is_dir { "dir" } else { "file" }
        }));
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::{Value, json};
    use slab_agent::{ToolContext, ToolHandler};

    use super::*;

    fn ctx() -> ToolContext {
        ToolContext { thread_id: "thread".into(), turn_index: 0, depth: 0 }
    }

    #[tokio::test]
    async fn file_glob_matches_files_and_respects_gitignore() {
        let root = temp_root("matches_gitignore");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::create_dir_all(root.join("ignored")).expect("create ignored");
        fs::write(root.join(".gitignore"), "ignored/\n").expect("write gitignore");
        fs::write(root.join("src").join("lib.rs"), "").expect("write rust file");
        fs::write(root.join("src").join("main.ts"), "").expect("write ts file");
        fs::write(root.join("ignored").join("skip.rs"), "").expect("write ignored file");
        let tool = FileGlobTool::new(Some(root.clone()));

        let output = tool
            .execute(&ctx(), &json!({"path": ".", "pattern": "*.rs"}))
            .await
            .expect("glob output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");
        let matches = value["matches"].as_array().expect("matches");

        assert_eq!(matches.len(), 1);
        assert!(
            matches[0]["path"].as_str().expect("path").ends_with("src\\lib.rs")
                || matches[0]["path"].as_str().expect("path").ends_with("src/lib.rs")
        );
        assert_eq!(matches[0]["kind"], "file");
        assert_eq!(value["truncated"], false);

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn file_glob_caps_results_and_marks_truncation() {
        let root = temp_root("truncated");
        for idx in 0..3 {
            fs::write(root.join(format!("{idx}.txt")), "").expect("write file");
        }
        let tool = FileGlobTool::new(Some(root.clone()));

        let output = tool
            .execute(&ctx(), &json!({"path": ".", "pattern": "*.txt", "max_results": 1}))
            .await
            .expect("glob output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert_eq!(value["matches"].as_array().expect("matches").len(), 1);
        assert_eq!(value["total"], 1);
        assert_eq!(value["truncated"], true);

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn file_glob_rejects_invalid_glob() {
        let root = temp_root("invalid_glob");
        let tool = FileGlobTool::new(Some(root.clone()));

        let error =
            tool.execute(&ctx(), &json!({"path": ".", "pattern": "["})).await.expect_err("glob");

        assert!(error.to_string().contains("invalid glob"));
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn file_glob_rejects_workspace_escape_before_scanning() {
        let root = temp_root("escape");
        let tool = FileGlobTool::new(Some(root.clone()));

        let error = tool
            .execute(&ctx(), &json!({"path": "../outside", "pattern": "*"}))
            .await
            .expect_err("escape rejected");

        assert!(error.to_string().contains("workspace path"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_glob_schema_matches_required_arguments() {
        let schema = FileGlobTool::new(None).parameters_schema();

        assert_eq!(schema["properties"]["pattern"]["type"], "string");
        assert_eq!(schema["properties"]["path"]["default"], ".");
        assert_eq!(schema["properties"]["max_results"]["default"], 200);
        assert_eq!(schema["properties"]["include_dirs"]["default"], false);
        assert_eq!(schema["required"], json!(["pattern"]));
    }

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "slab_agent_tools_glob_{name}_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }
}
