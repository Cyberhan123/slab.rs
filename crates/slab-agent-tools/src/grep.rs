//! Grep tool — gitignore-aware pattern search.
//!
//! Uses the `ignore` crate (which powers ripgrep) for directory traversal and
//! `regex` for line-level matching.

use std::path::PathBuf;

use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;
use slab_agent::{AgentError, ToolApprovalRequest, ToolContext, ToolHandler, ToolOutput};

use crate::sensitive_path::approval_for_values;

const DEFAULT_MAX_RESULTS: usize = 200;
const HARD_MAX_RESULTS: usize = 1000;
const MAX_CONTEXT_LINES: usize = 10;

/// Search files for lines matching a regular expression.
///
/// # JSON schema
///
/// ```json
/// {
///   "pattern": "fn execute",
///   "path": ".",
///   "glob": "*.rs",          // optional
///   "case_insensitive": false, // optional
///   "max_results": 200,        // optional
///   "context_lines": 0         // optional
/// }
/// ```
///
/// Returns matches as `[{file, line, text}]`.
pub struct GrepTool {
    workspace_root: Option<PathBuf>,
    extra_roots: Vec<PathBuf>,
}

impl GrepTool {
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
                },
                "max_results": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 1000,
                    "default": 200
                },
                "context_lines": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 10,
                    "description": "Number of surrounding lines to include before and after each match.",
                    "default": 0
                }
            },
            "required": ["pattern"]
        })
    }

    fn approval_request(&self, arguments: &Value) -> Option<ToolApprovalRequest> {
        approval_for_values(
            "grep",
            &[
                ("path", arguments.get("path").and_then(Value::as_str)),
                ("glob", arguments.get("glob").and_then(Value::as_str)),
                ("pattern", arguments.get("pattern").and_then(Value::as_str)),
            ],
        )
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

        let path_str = arguments.get("path").and_then(Value::as_str).unwrap_or(".");

        let glob_str = arguments.get("glob").and_then(Value::as_str).map(str::to_owned);
        let case_insensitive =
            arguments.get("case_insensitive").and_then(Value::as_bool).unwrap_or(false);
        let max_results = arguments
            .get("max_results")
            .and_then(Value::as_u64)
            .map(|value| value.clamp(1, HARD_MAX_RESULTS as u64) as usize)
            .unwrap_or(DEFAULT_MAX_RESULTS);
        let context_lines = arguments
            .get("context_lines")
            .and_then(Value::as_u64)
            .map(|value| value.min(MAX_CONTEXT_LINES as u64) as usize)
            .unwrap_or(0);

        let search_root = crate::fs::resolve_agent_path(
            self.workspace_root.as_deref(),
            &self.extra_roots,
            path_str,
        )?;

        // Build the regex.
        let re = regex::RegexBuilder::new(&pattern)
            .case_insensitive(case_insensitive)
            .build()
            .map_err(|e| AgentError::ToolExecution(format!("invalid regex '{pattern}': {e}")))?;

        // Run the blocking scan on a dedicated thread so we don't block the async runtime.
        let results = tokio::task::spawn_blocking(move || {
            grep_blocking(&search_root, &re, glob_str.as_deref(), max_results, context_lines)
        })
        .await
        .map_err(|e| AgentError::ToolExecution(format!("grep task panicked: {e}")))?;

        let results =
            results.map_err(|e| AgentError::ToolExecution(format!("grep failed: {e}")))?;

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

fn grep_blocking(
    root: &std::path::Path,
    re: &Regex,
    glob: Option<&str>,
    max_results: usize,
    context_lines: usize,
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
        if results.len() >= max_results {
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
        let lines = content.lines().collect::<Vec<_>>();
        for (idx, line) in lines.iter().copied().enumerate() {
            if results.len() >= max_results {
                break;
            }
            if re.is_match(line) {
                let mut match_payload = serde_json::json!({
                    "file": path.display().to_string(),
                    "line": idx + 1,
                    "text": line
                });
                if context_lines > 0 {
                    let before_start = idx.saturating_sub(context_lines);
                    let after_end = (idx + 1 + context_lines).min(lines.len());
                    match_payload["before_context"] = serde_json::json!(
                        (before_start..idx)
                            .map(|line_idx| serde_json::json!({
                                "line": line_idx + 1,
                                "text": lines[line_idx]
                            }))
                            .collect::<Vec<_>>()
                    );
                    match_payload["after_context"] = serde_json::json!(
                        ((idx + 1)..after_end)
                            .map(|line_idx| serde_json::json!({
                                "line": line_idx + 1,
                                "text": lines[line_idx]
                            }))
                            .collect::<Vec<_>>()
                    );
                }
                results.push(match_payload);
            }
        }
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
        ToolContext::for_thread("thread").build()
    }

    #[tokio::test]
    async fn grep_tool_filters_by_glob_and_case() {
        let root = temp_root("filters");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(root.join("src").join("lib.rs"), "Alpha\nbeta\n").expect("write rust file");
        fs::write(root.join("notes.txt"), "alpha\n").expect("write text file");
        let tool = GrepTool::new(Some(root.clone()));

        let output = tool
            .execute(
                &ctx(),
                &json!({
                    "path": ".",
                    "pattern": "alpha",
                    "glob": "*.rs",
                    "case_insensitive": true
                }),
            )
            .await
            .expect("grep output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert_eq!(value["total"], 1);
        assert_eq!(value["truncated"], false);
        assert_eq!(value["matches"][0]["line"], 1);
        assert_eq!(value["matches"][0]["text"], "Alpha");
        assert!(value["matches"][0]["file"].as_str().expect("file").ends_with("lib.rs"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn grep_tool_requires_approval_for_sensitive_path_glob_or_pattern() {
        let tool = GrepTool::new(Some(PathBuf::from(".")));

        assert!(tool.approval_request(&json!({"path": ".env", "pattern": "KEY"})).is_some());
        assert!(tool.approval_request(&json!({"path": ".", "pattern": "token"})).is_some());
        assert!(
            tool.approval_request(&json!({"path": ".", "pattern": "KEY", "glob": "*.pem"}))
                .is_some()
        );
        assert!(
            tool.approval_request(&json!({"path": "src", "pattern": "tokenization"})).is_none()
        );
    }

    #[tokio::test]
    async fn grep_tool_requires_pattern_argument() {
        let root = temp_root("missing_pattern");
        let tool = GrepTool::new(Some(root.clone()));

        let error = tool.execute(&ctx(), &json!({"path": "."})).await.expect_err("missing pattern");

        assert_eq!(error.to_string(), "tool execution error: missing 'pattern' argument");
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn grep_tool_rejects_workspace_escape_before_scanning() {
        let root = temp_root("escape");
        let tool = GrepTool::new(Some(root.clone()));

        let parent_escape = tool
            .execute(&ctx(), &json!({"path": "../outside/missing.txt", "pattern": "needle"}))
            .await
            .expect_err("parent escape rejected");
        assert!(parent_escape.to_string().contains("workspace path"));

        let absolute_escape = tool
            .execute(
                &ctx(),
                &json!({"path": root.join("file.txt").display().to_string(), "pattern": "needle"}),
            )
            .await
            .expect_err("absolute path rejected");
        assert!(absolute_escape.to_string().contains("absolute path"));

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn grep_tool_reports_invalid_regex() {
        let root = temp_root("invalid_regex");
        let tool = GrepTool::new(Some(root.clone()));

        let error =
            tool.execute(&ctx(), &json!({"path": ".", "pattern": "["})).await.expect_err("regex");

        assert!(error.to_string().contains("invalid regex"));
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn grep_tool_reports_invalid_glob() {
        let root = temp_root("invalid_glob");
        let tool = GrepTool::new(Some(root.clone()));

        let error = tool
            .execute(&ctx(), &json!({"path": ".", "pattern": "needle", "glob": "["}))
            .await
            .expect_err("invalid glob");

        assert!(error.to_string().contains("invalid glob"));
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn grep_tool_without_workspace_allows_absolute_file_path() {
        let root = temp_root("absolute_without_workspace");
        let file = root.join("notes.txt");
        fs::write(&file, "needle\n").expect("write file");
        let tool = GrepTool::new(None);

        let output = tool
            .execute(&ctx(), &json!({"path": file.display().to_string(), "pattern": "needle"}))
            .await
            .expect("grep output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert_eq!(value["total"], 1);
        assert_eq!(value["matches"][0]["file"], file.display().to_string());
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn grep_tool_includes_hidden_files_and_skips_binary_content() {
        let root = temp_root("hidden_binary");
        fs::write(root.join(".env"), "TOKEN=needle\n").expect("write hidden file");
        fs::write(root.join("binary.bin"), [0xff, 0xfe, b'n', b'e', b'e', b'd', b'l', b'e'])
            .expect("write binary file");
        let tool = GrepTool::new(Some(root.clone()));

        let output = tool
            .execute(&ctx(), &json!({"path": ".", "pattern": "needle"}))
            .await
            .expect("grep output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert_eq!(value["total"], 1);
        assert!(value["matches"][0]["file"].as_str().expect("file").ends_with(".env"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn grep_tool_schema_matches_required_arguments() {
        let schema = GrepTool::new(None).parameters_schema();

        assert_eq!(schema["properties"]["pattern"]["type"], "string");
        assert_eq!(schema["properties"]["path"]["default"], ".");
        assert_eq!(schema["properties"]["case_insensitive"]["default"], false);
        assert_eq!(schema["properties"]["max_results"]["default"], 200);
        assert_eq!(schema["properties"]["context_lines"]["default"], 0);
        assert_eq!(schema["required"], json!(["pattern"]));
    }

    #[tokio::test]
    async fn grep_tool_caps_results_and_marks_truncation() {
        let root = temp_root("truncated");
        let content =
            std::iter::repeat_n("hit", DEFAULT_MAX_RESULTS + 5).collect::<Vec<_>>().join("\n");
        fs::write(root.join("many.txt"), format!("{content}\n")).expect("write matches");
        let tool = GrepTool::new(Some(root.clone()));

        let output = tool
            .execute(&ctx(), &json!({"path": "many.txt", "pattern": "hit"}))
            .await
            .expect("grep output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert_eq!(value["matches"].as_array().expect("matches").len(), DEFAULT_MAX_RESULTS);
        assert_eq!(value["total"], DEFAULT_MAX_RESULTS);
        assert_eq!(value["truncated"], true);

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn grep_tool_can_return_context_lines_and_custom_limit() {
        let root = temp_root("context");
        fs::write(root.join("notes.txt"), "before\nneedle\nafter\nneedle\n").expect("write file");
        let tool = GrepTool::new(Some(root.clone()));

        let output = tool
            .execute(
                &ctx(),
                &json!({
                    "path": ".",
                    "pattern": "needle",
                    "context_lines": 1,
                    "max_results": 1
                }),
            )
            .await
            .expect("grep output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert_eq!(value["total"], 1);
        assert_eq!(value["truncated"], true);
        assert_eq!(value["matches"][0]["line"], 2);
        assert_eq!(value["matches"][0]["before_context"], json!([{ "line": 1, "text": "before" }]));
        assert_eq!(value["matches"][0]["after_context"], json!([{ "line": 3, "text": "after" }]));

        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "slab_agent_tools_grep_{name}_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }
}
