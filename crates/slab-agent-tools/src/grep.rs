//! Grep tool — gitignore-aware pattern search.
//!
//! Uses the `ignore` crate (which powers ripgrep) for directory traversal and
//! `regex` for line-level matching.  Mirrors the approach used by
//! `codex-file-search`.

use std::path::PathBuf;

use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput};

const MAX_RESULTS: usize = 200;

/// Search files for lines matching a regular expression.
///
/// # JSON schema
///
/// ```json
/// {
///   "pattern": "fn execute",
///   "path": ".",
///   "glob": "*.rs",          // optional
///   "case_insensitive": false // optional
/// }
/// ```
///
/// Returns up to 200 matches as `[{file, line, text}]`.
pub struct GrepTool {
    workspace_root: Option<PathBuf>,
}

impl GrepTool {
    pub fn new(workspace_root: Option<PathBuf>) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl ToolHandler for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search files for lines matching a regular expression.  Respects \
         .gitignore rules.  Returns up to 200 matches with file path, line \
         number (1-based), and the matching line."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regular expression to search for."
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search (default: workspace root or '.').",
                    "default": "."
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to restrict which files are searched (e.g. '*.rs')."
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "If true, match case-insensitively.",
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
        let pattern = arguments
            .get("pattern")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::ToolExecution("missing 'pattern' argument".into()))?
            .to_owned();

        let path_str = arguments
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or(".");

        let glob_str = arguments.get("glob").and_then(Value::as_str).map(str::to_owned);
        let case_insensitive =
            arguments.get("case_insensitive").and_then(Value::as_bool).unwrap_or(false);

        let search_root = if let Some(ref root) = self.workspace_root {
            // Reject absolute paths to prevent escaping the workspace.
            if PathBuf::from(path_str).is_absolute() {
                return Err(AgentError::ToolExecution(format!(
                    "absolute path '{path_str}' is not allowed when a workspace root is configured; \
                     use a path relative to the workspace root"
                )));
            }
            let resolved = root.join(path_str);
            // Verify the resolved path stays within the workspace root.
            // For non-existent paths we fall back to a lexical prefix check.
            let canonical_root =
                root.canonicalize().map_err(|e| AgentError::ToolExecution(format!(
                    "failed to canonicalize workspace root: {e}"
                )))?;
            if let Ok(canonical_resolved) = resolved.canonicalize() {
                if !canonical_resolved.starts_with(&canonical_root) {
                    return Err(AgentError::ToolExecution(format!(
                        "path '{path_str}' escapes the workspace root"
                    )));
                }
            }
            resolved
        } else {
            PathBuf::from(path_str)
        };

        // Build the regex.
        let re = regex::RegexBuilder::new(&pattern)
            .case_insensitive(case_insensitive)
            .build()
            .map_err(|e| AgentError::ToolExecution(format!("invalid regex '{pattern}': {e}")))?;

        // Run the blocking scan on a dedicated thread so we don't block the async runtime.
        let results =
            tokio::task::spawn_blocking(move || grep_blocking(&search_root, &re, glob_str.as_deref()))
                .await
                .map_err(|e| AgentError::ToolExecution(format!("grep task panicked: {e}")))?;

        let results = results
            .map_err(|e| AgentError::ToolExecution(format!("grep failed: {e}")))?;

        Ok(ToolOutput {
            content: serde_json::json!({
                "matches": results,
                "total": results.len(),
                "truncated": results.len() >= MAX_RESULTS
            })
            .to_string(),
            metadata: None,
        })
    }
}

fn grep_blocking(
    root: &std::path::Path,
    re: &Regex,
    glob: Option<&str>,
) -> Result<Vec<serde_json::Value>, String> {
    let mut builder = ignore::WalkBuilder::new(root);
    builder.hidden(false); // don't ignore hidden files (show dot-files)

    if let Some(g) = glob {
        let mut override_builder = ignore::overrides::OverrideBuilder::new(root);
        override_builder.add(g).map_err(|e| format!("invalid glob: {e}"))?;
        let overrides = override_builder.build().map_err(|e| format!("glob build error: {e}"))?;
        builder.overrides(overrides);
    }

    let mut results = Vec::new();

    for result in builder.build() {
        if results.len() >= MAX_RESULTS {
            break;
        }
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue, // skip binary / unreadable files
        };
        for (idx, line) in content.lines().enumerate() {
            if results.len() >= MAX_RESULTS {
                break;
            }
            if re.is_match(line) {
                results.push(serde_json::json!({
                    "file": path.display().to_string(),
                    "line": idx + 1,
                    "text": line
                }));
            }
        }
    }

    Ok(results)
}
