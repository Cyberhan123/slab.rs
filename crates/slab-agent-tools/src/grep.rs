//! Grep tool — gitignore-aware pattern search.
//!
//! Uses the `ignore` crate (which powers ripgrep) for directory traversal and
//! `regex` for line-level matching.

use std::path::PathBuf;

use async_trait::async_trait;
use regex::Regex;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolOutput, TypedTool, typed_input_schema};
use slab_utils::string::{truncate_line_bytes, truncate_middle_bytes};

const DEFAULT_MAX_RESULTS: usize = 200;
const HARD_MAX_RESULTS: usize = 1000;
const MAX_CONTEXT_LINES: usize = 10;

/// Cap on a single matched line — the "matched line" of a minified or
/// generated file can itself be the whole file (measured up to ~900KB in
/// dangling git objects).
const MAX_LINE_BYTES: usize = 2 * 1024;
/// Cap on one serialized match payload (text + context arrays).
const MAX_MATCH_PREVIEW_BYTES: usize = 4 * 1024;
/// Cap on the serialized match list inside the response envelope.
const MAX_RESPONSE_BYTES: usize = 64 * 1024;
/// Headroom inside [`MAX_RESPONSE_BYTES`] for the envelope keys themselves.
const RESPONSE_ENVELOPE_MARGIN_BYTES: usize = 1024;

/// Arguments for the `grep` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GrepArgs {
    /// Regular expression to search for.
    pattern: String,
    /// Directory or file to search (default: workspace root or '.').
    #[serde(default = "default_path")]
    path: String,
    /// Glob pattern to restrict which files are searched (e.g. '*.rs'). Negated patterns are not supported. Files ignored by .gitignore are always excluded regardless of the glob.
    glob: Option<String>,
    /// If true, match case-insensitively.
    #[serde(default)]
    case_insensitive: bool,
    #[serde(default = "default_max_results")]
    #[schemars(range(min = 1, max = 1000))]
    max_results: u64,
    /// Number of surrounding lines to include before and after each match.
    #[serde(default)]
    #[schemars(range(min = 0, max = 10))]
    context_lines: u64,
}

fn default_path() -> String {
    ".".to_owned()
}

fn default_max_results() -> u64 {
    DEFAULT_MAX_RESULTS as u64
}

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
impl TypedTool for GrepTool {
    type Input = GrepArgs;
    fn name(&self) -> &str {
        "grep"
    }

    /// Pure read — safe to run concurrently with other read-only calls.
    fn is_concurrency_safe(&self, _arguments: &serde_json::Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Search files for lines matching a regular expression.  Respects \
         .gitignore rules and skips .git, node_modules, target, vendor, dist, \
         lockfiles, and cargo-bazel generated files by default.  Returns up to \
         200 matches with file path, line number (1-based), and the matching \
         line (very long lines and oversized result sets are truncated with \
         explicit markers)."
    }

    fn parameters_schema(&self) -> Value {
        typed_input_schema::<GrepArgs>()
    }

    fn describe_operation(&self, arguments: &Value) -> Option<slab_agent::OperationDescriptor> {
        let path = arguments.get("path").and_then(Value::as_str).unwrap_or(".");
        let pattern = arguments.get("pattern").and_then(Value::as_str).unwrap_or("");
        Some(
            slab_agent::OperationDescriptor::read_only(format!("{path}:{pattern}"))
                .with_workspace(self.workspace_root.clone()),
        )
    }

    async fn execute(&self, ctx: &ToolContext, args: GrepArgs) -> Result<ToolOutput, AgentError> {
        let max_results = args.max_results.clamp(1, HARD_MAX_RESULTS as u64) as usize;
        let context_lines = args.context_lines.min(MAX_CONTEXT_LINES as u64) as usize;

        let search_root = crate::fs::resolve_agent_path(
            self.workspace_root.as_deref(),
            &self.extra_roots,
            &args.path,
        )?;

        // Build the regex.
        let re = regex::RegexBuilder::new(&args.pattern)
            .case_insensitive(args.case_insensitive)
            .build()
            .map_err(|e| {
                AgentError::ToolExecution(format!("invalid regex '{}': {e}", args.pattern))
            })?;

        // Run the blocking scan on a dedicated thread so we don't block the async runtime.
        let scan = tokio::task::spawn_blocking(move || {
            grep_blocking(&search_root, &re, args.glob.as_deref(), max_results, context_lines)
        })
        .await
        .map_err(|e| AgentError::ToolExecution(format!("grep task panicked: {e}")))?;

        let scan = scan.map_err(|e| AgentError::ToolExecution(format!("grep failed: {e}")))?;

        // Spill the full (individually-capped) match list when the response
        // budget dropped matches — the model can read the artifact for the
        // complete set instead of re-running a narrower search blind.
        let full_results_artifact = if scan.omitted > 0 {
            let nonce = {
                use std::hash::{DefaultHasher, Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                args.pattern.hash(&mut hasher);
                args.path.hash(&mut hasher);
                format!("{:016x}", hasher.finish())
            };
            match serde_json::to_vec_pretty(&scan.all_matches) {
                Ok(bytes) => {
                    crate::artifact::write_tool_artifact(
                        ctx.workspace.as_ref().map(|workspace| workspace.root.as_path()),
                        &ctx.thread_id,
                        &format!("grep-results-{nonce}-t{}.json", ctx.turn_index),
                        &bytes,
                    )
                    .await
                }
                Err(_) => None,
            }
        } else {
            None
        };

        let mut envelope = serde_json::json!({
            "matches": scan.matches,
            "total": scan.matches.len(),
            "truncated": scan.found >= max_results || scan.omitted > 0,
            "omitted_matches": scan.omitted
        });
        if let Some(reference) = full_results_artifact {
            envelope["full_results_artifact"] = serde_json::json!(reference);
        }

        Ok(ToolOutput { content: envelope.to_string(), metadata: None })
    }
}

/// Byte-bounded scan outcome: every match is counted (`found`) and collected
/// with its individual caps (`all_matches`), but only as many as fit the
/// response byte budget are injected (`matches`); the rest are reported via
/// `omitted`. Which matches are found is unchanged — the count/byte caps only
/// bound how much of them reaches the model.
struct GrepScan {
    matches: Vec<serde_json::Value>,
    all_matches: Vec<serde_json::Value>,
    found: usize,
    omitted: usize,
}

fn grep_blocking(
    root: &std::path::Path,
    re: &Regex,
    glob: Option<&str>,
    max_results: usize,
    context_lines: usize,
) -> Result<GrepScan, String> {
    // The optional `glob` argument filters WITH POST-PROCESSING rather than an
    // `ignore` Override whitelist: overrides take precedence over .gitignore,
    // so `glob: "**/*"` used to match ignored directories themselves and leak
    // them into the search. Post-filtering keeps .gitignore authoritative.
    let glob_matcher = match glob {
        Some(g) => {
            Some(globset::Glob::new(g).map_err(|e| format!("invalid glob: {e}"))?.compile_matcher())
        }
        None => None,
    };

    let mut builder = ignore::WalkBuilder::new(root);
    builder.hidden(false); // don't ignore hidden files (show dot-files)
    builder.require_git(false); // apply .gitignore even outside a git repo (matches file_glob)
    // Prune non-source trees by name during traversal (see `exclusions`);
    // never via an Override whitelist, which would beat .gitignore.
    builder.filter_entry(|entry| !crate::exclusions::is_default_excluded(entry));

    let mut scan = GrepScan { matches: Vec::new(), all_matches: Vec::new(), found: 0, omitted: 0 };
    let payload_budget = MAX_RESPONSE_BYTES.saturating_sub(RESPONSE_ENVELOPE_MARGIN_BYTES);
    let mut serialized_bytes = 0usize;

    for result in builder.build() {
        if scan.found >= max_results {
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
        if let Some(matcher) = glob_matcher.as_ref() {
            let rel = path.strip_prefix(root).unwrap_or(path);
            let candidate = if rel.as_os_str().is_empty() {
                std::path::Path::new(path.file_name().unwrap_or_default())
            } else {
                rel
            };
            if !matcher.is_match(candidate) {
                continue;
            }
        }
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue, // skip binary / unreadable files
        };
        let lines = content.lines().collect::<Vec<_>>();
        for (idx, line) in lines.iter().copied().enumerate() {
            if scan.found >= max_results {
                break;
            }
            if re.is_match(line) {
                scan.found += 1;
                let payload = build_match_payload(path, idx, line, &lines, context_lines);
                let serialized = serde_json::to_string(&payload).unwrap_or_default();
                scan.all_matches.push(payload);
                if serialized_bytes + serialized.len() <= payload_budget {
                    serialized_bytes += serialized.len();
                    scan.matches.push(scan.all_matches[scan.all_matches.len() - 1].clone());
                } else {
                    scan.omitted += 1;
                }
            }
        }
    }

    Ok(scan)
}

/// Build one match payload with per-line and per-match byte caps. Context
/// arrays are the first thing dropped when the preview cap is hit; the match
/// text itself keeps head and tail with an explicit omission marker.
fn build_match_payload(
    path: &std::path::Path,
    idx: usize,
    line: &str,
    lines: &[&str],
    context_lines: usize,
) -> serde_json::Value {
    let text = truncate_line_bytes(line, MAX_LINE_BYTES);
    let mut payload = serde_json::json!({
        "file": path.display().to_string(),
        "line": idx + 1,
        "text": text,
    });
    if context_lines > 0 {
        let before_start = idx.saturating_sub(context_lines);
        let after_end = (idx + 1 + context_lines).min(lines.len());
        payload["before_context"] = serde_json::json!(
            (before_start..idx)
                .map(|line_idx| context_entry(line_idx, lines[line_idx]))
                .collect::<Vec<_>>()
        );
        payload["after_context"] = serde_json::json!(
            ((idx + 1)..after_end)
                .map(|line_idx| context_entry(line_idx, lines[line_idx]))
                .collect::<Vec<_>>()
        );
    }

    let serialized = serde_json::to_string(&payload).unwrap_or_default();
    if serialized.len() > MAX_MATCH_PREVIEW_BYTES {
        if let Some(obj) = payload.as_object_mut() {
            obj.remove("before_context");
            obj.remove("after_context");
        }
        // Still over (very long path / text): middle-truncate the text to
        // whatever budget the rest of the payload leaves.
        let current_text = payload["text"].as_str().map(str::to_owned).unwrap_or_default();
        let overhead = serde_json::to_string(&payload)
            .unwrap_or_default()
            .len()
            .saturating_sub(current_text.len());
        let text_budget = MAX_MATCH_PREVIEW_BYTES.saturating_sub(overhead).max(256);
        let (bounded, _) = truncate_middle_bytes(&current_text, text_budget, 0.7);
        payload["text"] = serde_json::json!(bounded);
        payload["truncated"] = serde_json::json!({ "text_bytes_total": line.len() });
    }
    payload
}

fn context_entry(line_idx: usize, text: &str) -> serde_json::Value {
    serde_json::json!({
        "line": line_idx + 1,
        "text": truncate_line_bytes(text, MAX_LINE_BYTES),
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

        let output = ToolHandler::execute(
            &tool,
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
    fn grep_tool_describes_read_only_operation() {
        let tool = GrepTool::new(Some(PathBuf::from(".")));

        let desc = ToolHandler::describe_operation(
            &tool,
            &json!({"path": "src", "pattern": "fn execute"}),
        )
        .expect("descriptor");
        assert_eq!(desc.category, slab_agent::OperationCategory::ReadOnly);
        assert_eq!(desc.subject, "src:fn execute");
    }

    #[tokio::test]
    async fn grep_tool_requires_pattern_argument() {
        let root = temp_root("missing_pattern");
        let tool = GrepTool::new(Some(root.clone()));

        let error = ToolHandler::execute(&tool, &ctx(), &json!({"path": "."}))
            .await
            .expect_err("missing pattern");

        assert_eq!(error.to_string(), "tool execution error: missing 'pattern' argument");
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn grep_tool_rejects_workspace_escape_before_scanning() {
        let root = temp_root("escape");
        let tool = GrepTool::new(Some(root.clone()));

        let parent_escape = ToolHandler::execute(
            &tool,
            &ctx(),
            &json!({"path": "../outside/missing.txt", "pattern": "needle"}),
        )
        .await
        .expect_err("parent escape rejected");
        assert!(parent_escape.to_string().contains("workspace path"));

        let absolute_escape = ToolHandler::execute(
            &tool,
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

        let error = ToolHandler::execute(&tool, &ctx(), &json!({"path": ".", "pattern": "["}))
            .await
            .expect_err("regex");

        assert!(error.to_string().contains("invalid regex"));
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn grep_tool_reports_invalid_glob() {
        let root = temp_root("invalid_glob");
        let tool = GrepTool::new(Some(root.clone()));

        let error = ToolHandler::execute(
            &tool,
            &ctx(),
            &json!({"path": ".", "pattern": "needle", "glob": "["}),
        )
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

        let output = ToolHandler::execute(
            &tool,
            &ctx(),
            &json!({"path": file.display().to_string(), "pattern": "needle"}),
        )
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

        let output =
            ToolHandler::execute(&tool, &ctx(), &json!({"path": ".", "pattern": "needle"}))
                .await
                .expect("grep output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert_eq!(value["total"], 1);
        assert!(value["matches"][0]["file"].as_str().expect("file").ends_with(".env"));
        let _ = fs::remove_dir_all(root);
    }

    /// P3 regression: a `glob` broad enough to match an ignored directory name
    /// itself (`**/*`) used to short-circuit past .gitignore via the override
    /// whitelist and search ignored trees. Also pins `require_git(false)`:
    /// .gitignore applies even without a `.git` directory (none is created
    /// here, or in any other temp-root test).
    #[tokio::test]
    async fn grep_tool_glob_does_not_override_gitignore() {
        let root = temp_root("glob_gitignore");
        fs::create_dir_all(root.join("ignored")).expect("create ignored");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(root.join(".gitignore"), "ignored/\n").expect("write gitignore");
        fs::write(root.join("ignored").join("x.rs"), "needle\n").expect("write ignored file");
        fs::write(root.join("src").join("a.rs"), "needle\n").expect("write source file");
        let tool = GrepTool::new(Some(root.clone()));

        // With the broad glob the ignored tree must still be excluded.
        let output = ToolHandler::execute(
            &tool,
            &ctx(),
            &json!({"path": ".", "pattern": "needle", "glob": "**/*"}),
        )
        .await
        .expect("grep output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");
        let files: Vec<String> = value["matches"]
            .as_array()
            .expect("matches")
            .iter()
            .map(|m| m["file"].as_str().expect("file").to_string())
            .collect();
        assert!(!files.iter().any(|f| f.contains("ignored")), "ignored leaked: {files:?}");
        assert_eq!(value["total"], 1);

        // Without a glob the same gitignore filtering applies.
        let output =
            ToolHandler::execute(&tool, &ctx(), &json!({"path": ".", "pattern": "needle"}))
                .await
                .expect("grep output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");
        assert_eq!(value["total"], 1);
        assert!(value["matches"][0]["file"].as_str().expect("file").contains("src"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn grep_tool_schema_matches_required_arguments() {
        let schema = ToolHandler::parameters_schema(&GrepTool::new(None));

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

        let output =
            ToolHandler::execute(&tool, &ctx(), &json!({"path": "many.txt", "pattern": "hit"}))
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

        let output = ToolHandler::execute(
            &tool,
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

    #[tokio::test]
    async fn grep_tool_excludes_git_and_build_dirs_by_default() {
        let root = temp_root("default_excludes");
        // Dangling-object repro: one enormous single-line "blob" whose content
        // is a whole generated file (crates.bzl-style).
        let blob_dir = root.join(".git").join("lost-found").join("other");
        fs::create_dir_all(&blob_dir).expect("create blob dir");
        let blob_line = format!("{} needle BLOBSENTINEL", "# generated crates.bzl ".repeat(60_000));
        fs::write(blob_dir.join("0148550a6c54ff03a4351feb6ecfbf14cf28aed1"), &blob_line)
            .expect("write blob");
        for dir in ["node_modules", "target/debug", "vendor", "dist"] {
            fs::create_dir_all(root.join(dir)).expect("create excluded dir");
        }
        fs::write(root.join("node_modules").join("x.js"), "needle node_modules\n")
            .expect("write node_modules file");
        fs::write(root.join("target").join("debug").join("o.rs"), "needle target\n")
            .expect("write target file");
        fs::write(root.join("vendor").join("v.rs"), "needle vendor\n").expect("write vendor file");
        fs::write(root.join("dist").join("d.js"), "needle dist\n").expect("write dist file");
        fs::write(root.join("Cargo.lock"), "needle lock\n").expect("write lockfile");
        fs::write(root.join("crates.bzl"), "needle bazel\n").expect("write bazel file");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(root.join("src").join("a.rs"), "fn main() { needle }\n").expect("write source");
        let tool = GrepTool::new(Some(root.clone()));

        let output =
            ToolHandler::execute(&tool, &ctx(), &json!({"path": ".", "pattern": "needle"}))
                .await
                .expect("grep output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert!(
            output.content.len() < 64 * 1024,
            "response must stay under the 64KB budget, got {}",
            output.content.len()
        );
        assert!(!output.content.contains("BLOBSENTINEL"), "dangling blob leaked");
        assert!(!output.content.contains("generated crates.bzl"), "blob content leaked");
        let files: Vec<String> = value["matches"]
            .as_array()
            .expect("matches")
            .iter()
            .map(|m| m["file"].as_str().expect("file").replace('\\', "/"))
            .collect();
        assert_eq!(files.len(), 1, "only src/a.rs should match: {files:?}");
        assert!(files[0].ends_with("src/a.rs"));
        assert_eq!(value["omitted_matches"], 0);

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn grep_tool_searches_explicitly_targeted_excluded_root() {
        let root = temp_root("explicit_excluded_root");
        let git_dir = root.join(".git").join("hooks");
        fs::create_dir_all(&git_dir).expect("create git dir");
        fs::write(git_dir.join("sample.txt"), "needle\n").expect("write file");
        let tool = GrepTool::new(Some(root.clone()));

        let output =
            ToolHandler::execute(&tool, &ctx(), &json!({"path": ".git", "pattern": "needle"}))
                .await
                .expect("grep output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert_eq!(value["total"], 1, "explicit path into .git must still search");

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn grep_tool_caps_long_lines_with_marker() {
        let root = temp_root("long_line");
        let long_line = format!("start {} needle end", "x".repeat(10_000));
        fs::write(root.join("minified.txt"), format!("{long_line}\n")).expect("write file");
        let tool = GrepTool::new(Some(root.clone()));

        let output =
            ToolHandler::execute(&tool, &ctx(), &json!({"path": ".", "pattern": "needle"}))
                .await
                .expect("grep output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        let text = value["matches"][0]["text"].as_str().expect("text");
        assert!(text.len() < MAX_LINE_BYTES + 128, "line not capped: {}", text.len());
        let expected_total = long_line.len();
        assert!(
            text.contains(&format!("[...line truncated, {expected_total} bytes total]")),
            "marker missing: {text}"
        );
        assert!(text.starts_with("start "), "head must survive");

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn grep_tool_caps_response_bytes_and_reports_omitted() {
        let root = temp_root("response_budget");
        // ~60 fat matches, each capped near the 2KB line limit: the 64KB
        // response budget can only serialize a subset.
        let fat_line = format!("{} needle", "y".repeat(2_500));
        let content = (0..60).map(|_| fat_line.as_str()).collect::<Vec<_>>().join("\n");
        fs::write(root.join("fat.txt"), format!("{content}\n")).expect("write file");
        let tool = GrepTool::new(Some(root.clone()));

        let output = ToolHandler::execute(
            &tool,
            &ctx(),
            &json!({"path": ".", "pattern": "needle", "max_results": 60}),
        )
        .await
        .expect("grep output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert!(
            output.content.len() < MAX_RESPONSE_BYTES,
            "response must stay under the budget, got {}",
            output.content.len()
        );
        let serialized = value["matches"].as_array().expect("matches").len();
        let omitted = value["omitted_matches"].as_u64().expect("omitted") as usize;
        assert_eq!(serialized + omitted, 60, "counted matches must all be accounted for");
        assert!(omitted >= 1, "byte budget should have dropped some matches");
        assert_eq!(value["truncated"], true);
        assert_eq!(value["total"], serialized);
        // Same files still represented: the first match is present and capped.
        assert!(value["matches"][0]["file"].as_str().expect("file").contains("fat.txt"));

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn grep_tool_keeps_semantics_for_ordinary_files() {
        let root = temp_root("ordinary_semantics");
        fs::create_dir_all(root.join("src").join("deep")).expect("create dirs");
        fs::write(root.join(".env"), "needle=env\n").expect("write hidden file");
        fs::write(root.join("src").join("deep").join("a.rs"), "one\nneedle\ntwo\n")
            .expect("write a");
        fs::write(root.join("src").join("b.txt"), "needle first\nplain\nneedle last\n")
            .expect("write b");
        let tool = GrepTool::new(Some(root.clone()));

        let output =
            ToolHandler::execute(&tool, &ctx(), &json!({"path": ".", "pattern": "needle"}))
                .await
                .expect("grep output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        let mut hits: Vec<(String, u64, String)> = value["matches"]
            .as_array()
            .expect("matches")
            .iter()
            .map(|m| {
                (
                    m["file"].as_str().expect("file").replace('\\', "/"),
                    m["line"].as_u64().expect("line"),
                    m["text"].as_str().expect("text").to_string(),
                )
            })
            .collect();
        hits.sort();
        assert_eq!(hits.len(), 4, "all four ordinary matches must be found: {hits:?}");
        assert_eq!(value["truncated"], false);
        assert_eq!(value["omitted_matches"], 0);
        assert!(hits.iter().any(|(f, l, t)| f.ends_with(".env") && *l == 1 && t == "needle=env"));
        assert!(hits.iter().any(|(f, l, t)| f.ends_with("a.rs") && *l == 2 && t == "needle"));
        assert!(
            hits.iter().any(|(f, l, t)| f.ends_with("b.txt") && *l == 1 && t == "needle first")
        );
        assert!(hits.iter().any(|(f, l, t)| f.ends_with("b.txt") && *l == 3 && t == "needle last"));

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn grep_tool_spills_full_results_when_capped() {
        let root = temp_root("spill_full_results");
        let fat_line = format!("{} needle", "y".repeat(2_500));
        let content = (0..60).map(|_| fat_line.as_str()).collect::<Vec<_>>().join("\n");
        fs::write(root.join("fat.txt"), format!("{content}\n")).expect("write file");
        let tool = GrepTool::new(Some(root.clone()));
        let ctx = ToolContext::for_thread("spill-grep-thread")
            .workspace(slab_agent::WorkspaceRef { root: root.clone(), session_id: None })
            .build();

        let output = ToolHandler::execute(
            &tool,
            &ctx,
            &json!({"path": ".", "pattern": "needle", "max_results": 60}),
        )
        .await
        .expect("grep output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert!(value["omitted_matches"].as_u64().expect("omitted") > 0);
        let reference =
            value["full_results_artifact"].as_str().expect("full results artifact reference");
        assert!(reference.starts_with(".slab/artifacts/spill-grep-thread/grep-results-"));
        let artifact_path = root.join(reference);
        let spilled: Value =
            serde_json::from_str(&fs::read_to_string(artifact_path).expect("artifact exists"))
                .expect("artifact json");
        let spilled_matches = spilled.as_array().expect("match array");
        assert_eq!(spilled_matches.len(), 60, "artifact holds every counted match");

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
