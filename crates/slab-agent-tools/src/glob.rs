//! Gitignore-aware file glob tool.

use std::path::PathBuf;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use slab_agent::{
    AgentError, ToolContext, ToolHandler, ToolOutput, parse_tool_input, typed_input_schema,
};

const DEFAULT_MAX_RESULTS: usize = 200;
const HARD_MAX_RESULTS: usize = 1000;

/// Arguments for the `file_glob` tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct FileGlobArgs {
    /// Glob pattern relative to 'path', e.g. '*.rs' or 'src/**/*.ts'. Negated patterns are not supported. Files ignored by .gitignore are always excluded regardless of the pattern.
    pattern: String,
    /// Directory or file to search (default: workspace root or '.').
    #[serde(default = "default_path")]
    path: String,
    #[serde(default = "default_max_results")]
    #[schemars(range(min = 1, max = 1000))]
    max_results: u64,
    /// Whether matching directories should be included.
    #[serde(default)]
    include_dirs: bool,
}

fn default_path() -> String {
    ".".to_owned()
}

fn default_max_results() -> u64 {
    DEFAULT_MAX_RESULTS as u64
}

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

    /// Pure read — safe to run concurrently with other read-only calls.
    fn is_concurrency_safe(&self, _arguments: &serde_json::Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Find files by gitignore-aware glob pattern inside a workspace path. \
         Skips .git, node_modules, target, vendor, dist, lockfiles, and \
         cargo-bazel generated files by default."
    }

    fn parameters_schema(&self) -> Value {
        typed_input_schema::<FileGlobArgs>()
    }

    fn describe_operation(&self, arguments: &Value) -> Option<slab_agent::OperationDescriptor> {
        let path = arguments.get("path").and_then(Value::as_str).unwrap_or(".");
        let pattern = arguments.get("pattern").and_then(Value::as_str).unwrap_or("");
        Some(
            slab_agent::OperationDescriptor::read_only(format!("{path}/{pattern}"))
                .with_workspace(self.workspace_root.clone()),
        )
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let args = parse_tool_input::<FileGlobArgs>(arguments)?;
        let max_results = args.max_results.clamp(1, HARD_MAX_RESULTS as u64) as usize;
        let search_root = crate::fs::resolve_agent_path(
            self.workspace_root.as_deref(),
            &self.extra_roots,
            &args.path,
        )?;

        let results = tokio::task::spawn_blocking(move || {
            glob_blocking(&search_root, &args.pattern, max_results, args.include_dirs)
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
    // The user pattern filters WITH POST-PROCESSING, not via `ignore`'s
    // Override whitelist: overrides have the highest precedence in the `ignore`
    // crate, so a pattern like `**/*` would match an ignored directory itself,
    // short-circuit to Whitelist, and the .gitignore rule would never be
    // consulted (leaking e.g. `/binaries/` into the results). Walking with
    // plain gitignore filtering and matching the pattern afterwards keeps
    // .gitignore authoritative.
    let matcher = globset::Glob::new(pattern)
        .map_err(|error| format!("invalid glob: {error}"))?
        .compile_matcher();

    let mut builder = ignore::WalkBuilder::new(root);
    builder.hidden(false);
    builder.require_git(false);
    // Prune non-source trees by name during traversal (see `exclusions`),
    // shared with the grep tool so the two cannot drift.
    builder.filter_entry(|entry| !crate::exclusions::is_default_excluded(entry));

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
        if !glob_entry_matches(root, &entry, &matcher) {
            continue;
        }
        results.push(serde_json::json!({
            "path": entry.path().display().to_string(),
            "kind": if is_dir { "dir" } else { "file" }
        }));
    }

    Ok(results)
}

/// Match a walk entry against the user pattern, relative to the walk root.
/// When the root is a single file (empty relative path) fall back to the file
/// name, so `path: some_file.rs` + `pattern: '*.rs'` still matches.
fn glob_entry_matches(
    root: &std::path::Path,
    entry: &ignore::DirEntry,
    matcher: &globset::GlobMatcher,
) -> bool {
    let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
    let candidate =
        if rel.as_os_str().is_empty() { std::path::Path::new(entry.file_name()) } else { rel };
    matcher.is_match(candidate)
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

    #[test]
    fn file_glob_describes_read_only_operation() {
        let tool = FileGlobTool::new(Some(PathBuf::from(".")));

        let desc = tool
            .describe_operation(&json!({"path": "src", "pattern": "*.rs"}))
            .expect("descriptor");
        assert_eq!(desc.category, slab_agent::OperationCategory::ReadOnly);
        assert_eq!(desc.subject, "src/*.rs");
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

    /// P3 regression: `**/*` matches an ignored directory NAME itself, which
    /// used to short-circuit past the gitignore check via the override
    /// whitelist and leak the whole ignored tree into the results.
    #[tokio::test]
    async fn file_glob_double_star_does_not_leak_gitignored_directory() {
        let root = temp_root("double_star_gitignore");
        fs::create_dir_all(root.join("binaries")).expect("create binaries");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(root.join(".gitignore"), "binaries/\n").expect("write gitignore");
        fs::write(root.join("binaries").join("tool.exe"), "").expect("write ignored file");
        fs::write(root.join("src").join("a.rs"), "").expect("write source file");
        let tool = FileGlobTool::new(Some(root.clone()));

        let output = tool
            .execute(&ctx(), &json!({"path": ".", "pattern": "**/*"}))
            .await
            .expect("glob output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        let paths: Vec<String> = value["matches"]
            .as_array()
            .expect("matches")
            .iter()
            .map(|m| m["path"].as_str().expect("path").to_string())
            .collect();
        assert!(
            !paths.iter().any(|p| p.contains("binaries")),
            "ignored directory leaked: {paths:?}"
        );
        assert!(
            paths.iter().any(|p| p.replace('\\', "/").ends_with("src/a.rs")),
            "src/a.rs missing: {paths:?}"
        );
        let _ = fs::remove_dir_all(root);
    }

    /// Anchored gitignore patterns (`/build/`) must only ignore the root-level
    /// directory, not identically-named nested ones.
    #[tokio::test]
    async fn file_glob_anchored_gitignore_only_ignores_root_level_dir() {
        let root = temp_root("anchored_gitignore");
        fs::create_dir_all(root.join("build")).expect("create build");
        fs::create_dir_all(root.join("a").join("build")).expect("create nested build");
        fs::write(root.join(".gitignore"), "/build/\n").expect("write gitignore");
        fs::write(root.join("build").join("x.rs"), "").expect("write root-level file");
        fs::write(root.join("a").join("build").join("y.rs"), "").expect("write nested file");
        let tool = FileGlobTool::new(Some(root.clone()));

        let output = tool
            .execute(&ctx(), &json!({"path": ".", "pattern": "**/*.rs"}))
            .await
            .expect("glob output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        let paths: Vec<String> = value["matches"]
            .as_array()
            .expect("matches")
            .iter()
            .map(|m| m["path"].as_str().expect("path").replace('\\', "/"))
            .collect();
        assert!(
            !paths.iter().any(|p| p.ends_with("/build/x.rs")),
            "root-level build leaked: {paths:?}"
        );
        assert!(
            paths.iter().any(|p| p.ends_with("a/build/y.rs")),
            "nested build must survive: {paths:?}"
        );
        let _ = fs::remove_dir_all(root);
    }

    /// `*`-style patterns that can match the ignored directory name itself
    /// must still respect gitignore.
    #[tokio::test]
    async fn file_glob_star_pattern_matching_ignored_dir_name_excludes_it() {
        let root = temp_root("star_gitignore");
        fs::create_dir_all(root.join("ignored")).expect("create ignored");
        fs::write(root.join(".gitignore"), "ignored/\n").expect("write gitignore");
        fs::write(root.join("ignored").join("skip.rs"), "").expect("write ignored file");
        fs::write(root.join("keep.rs"), "").expect("write kept file");
        let tool = FileGlobTool::new(Some(root.clone()));

        let output = tool
            .execute(&ctx(), &json!({"path": ".", "pattern": "*", "include_dirs": true}))
            .await
            .expect("glob output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        let paths: Vec<String> = value["matches"]
            .as_array()
            .expect("matches")
            .iter()
            .map(|m| m["path"].as_str().expect("path").to_string())
            .collect();
        assert!(
            !paths.iter().any(|p| p.contains("ignored")),
            "ignored directory leaked through '*': {paths:?}"
        );
        assert!(
            paths.iter().any(|p| p.replace('\\', "/").ends_with("keep.rs")),
            "keep.rs missing: {paths:?}"
        );
        let _ = fs::remove_dir_all(root);
    }

    /// A single-file `path` still matches by file name (empty relative path
    /// falls back to `file_name`).
    #[tokio::test]
    async fn file_glob_matches_single_file_root() {
        let root = temp_root("single_file_root");
        let file = root.join("notes.md");
        fs::write(&file, "x").expect("write file");
        let tool = FileGlobTool::new(Some(root.clone()));

        let output = tool
            .execute(&ctx(), &json!({"path": "notes.md", "pattern": "*.md"}))
            .await
            .expect("glob output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        let matches = value["matches"].as_array().expect("matches");
        assert_eq!(matches.len(), 1);
        assert!(matches[0]["path"].as_str().expect("path").ends_with("notes.md"));
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

    #[tokio::test]
    async fn file_glob_excludes_git_and_vendor_by_default() {
        let root = temp_root("default_excludes");
        for dir in [".git/objects", "node_modules/pkg", "vendor/crate", "dist"] {
            fs::create_dir_all(root.join(dir)).expect("create excluded dir");
        }
        fs::write(root.join(".git").join("objects").join("x.txt"), "").expect("write git file");
        fs::write(root.join("node_modules").join("pkg").join("x.js"), "").expect("write nm file");
        fs::write(root.join("vendor").join("crate").join("x.rs"), "").expect("write vendor file");
        fs::write(root.join("dist").join("x.js"), "").expect("write dist file");
        fs::write(root.join("Cargo.lock"), "").expect("write lockfile");
        fs::write(root.join("crates.bzl"), "").expect("write bazel file");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(root.join("src").join("keep.rs"), "").expect("write kept file");
        let tool = FileGlobTool::new(Some(root.clone()));

        let output = tool
            .execute(&ctx(), &json!({"path": ".", "pattern": "**/*"}))
            .await
            .expect("glob output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        let paths: Vec<String> = value["matches"]
            .as_array()
            .expect("matches")
            .iter()
            .map(|m| m["path"].as_str().expect("path").replace('\\', "/"))
            .collect();
        for excluded in [".git/", "node_modules/", "vendor/", "dist/", "Cargo.lock", "crates.bzl"] {
            assert!(!paths.iter().any(|p| p.contains(excluded)), "{excluded} leaked: {paths:?}");
        }
        assert!(paths.iter().any(|p| p.ends_with("src/keep.rs")), "src/keep.rs missing: {paths:?}");

        let _ = fs::remove_dir_all(root);
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
