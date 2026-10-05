//! Read-only memory workspace tools for the standalone MCP server (codex
//! `MemoriesMcpServer` parity).
//!
//! Exposed via `bin/slab-mcp-server`: list project memory stores, list a
//! project's recall manifest, keyword-search it, and read whitelisted memory
//! files. Every read is chrooted to the per-project memory workspace:
//!
//! * the caller-supplied project key is re-sanitized through
//!   [`slab_agent_memories::fs::project_memory_root`], which collapses any
//!   non-`[a-z0-9]` run — traversal characters cannot survive it;
//! * summary filenames must pass [`recall::safe_summary_name`] (one flat
//!   `[A-Za-z0-9_.-]` segment, no hidden names, `..` included);
//! * the resolved path must canonicalize back inside the project root, so a
//!   symlink planted in the workspace cannot exfiltrate files outside it
//!   (the same defense-in-depth as `render_selected_entries`).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use slab_agent_memories::fs as memory_fs;
use slab_agent_memories::recall::{self, RecallManifestEntry};

pub const MEMORY_LIST_PROJECTS_TOOL: &str = "memory_list_projects";
pub const MEMORY_LIST_TOOL: &str = "memory_list";
pub const MEMORY_SEARCH_TOOL: &str = "memory_search";
pub const MEMORY_READ_TOOL: &str = "memory_read";

/// The `slab.source` stamped into `_meta` — the process exposing the tools.
const META_SOURCE: &str = "slab-mcp-server";
const MEMORY_PERMISSION: &str = "slab:mcp:memories:read";
/// Fixed read surface: the registry, the consolidated summary, and the raw
/// phase1 bodies; everything else must be a `rollout_summaries/<name>` entry.
const MEMORY_SUMMARY_FILE: &str = "memory_summary.md";
/// Env override for the memory root (default: `<app_home>/memories`, the same
/// default `bootstrap` resolves for the memory pipeline).
const MEMORY_ROOT_ENV: &str = "SLAB_MEMORIES_ROOT";

/// Every tool name this module serves, in listing order.
pub fn tool_names() -> &'static [&'static str] {
    &[MEMORY_LIST_PROJECTS_TOOL, MEMORY_LIST_TOOL, MEMORY_SEARCH_TOOL, MEMORY_READ_TOOL]
}

pub fn is_memory_tool(name: &str) -> bool {
    tool_names().contains(&name)
}

/// `tools/list` entries for the memory tools (same shape/annotations as the
/// server's `slab_server_info` spec).
pub fn tool_specs() -> Vec<Value> {
    vec![
        tool_spec(
            MEMORY_LIST_PROJECTS_TOOL,
            "List memory projects",
            "List the per-project memory stores under the Slab memory root.",
            json!({ "type": "object", "properties": {} }),
            json!({
                "type": "object",
                "properties": {
                    "memory_root": { "type": "string" },
                    "projects": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["memory_root", "projects"]
            }),
        ),
        tool_spec(
            MEMORY_LIST_TOOL,
            "List memory files",
            "List a project memory store's recall manifest (summary files with \
             title/keywords/freshness), newest first.",
            json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project key (from memory_list_projects)."
                    }
                },
                "required": ["project"]
            }),
            json!({
                "type": "object",
                "properties": {
                    "project": { "type": "string" },
                    "entries": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "filename": { "type": "string" },
                                "title": { "type": "string" },
                                "keywords": { "type": "string" },
                                "cwd": { "type": "string" },
                                "updated_at": { "type": "string" }
                            },
                            "required": ["filename", "title", "keywords", "cwd"]
                        }
                    }
                },
                "required": ["project", "entries"]
            }),
        ),
        tool_spec(
            MEMORY_SEARCH_TOOL,
            "Search memory files",
            "Case-insensitive keyword search over a project memory store's \
             manifest (filename, title, keywords, cwd).",
            json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project key (from memory_list_projects)."
                    },
                    "query": { "type": "string", "description": "Keyword to match." }
                },
                "required": ["project", "query"]
            }),
            json!({
                "type": "object",
                "properties": {
                    "project": { "type": "string" },
                    "query": { "type": "string" },
                    "matches": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "filename": { "type": "string" },
                                "title": { "type": "string" },
                                "keywords": { "type": "string" },
                                "cwd": { "type": "string" },
                                "updated_at": { "type": "string" }
                            },
                            "required": ["filename", "title", "keywords", "cwd"]
                        }
                    }
                },
                "required": ["project", "query", "matches"]
            }),
        ),
        tool_spec(
            MEMORY_READ_TOOL,
            "Read a memory file",
            "Read one memory file from a project store. Allowed: MEMORY.md, \
             memory_summary.md, raw_memories.md, or rollout_summaries/<name>.",
            json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project key (from memory_list_projects)."
                    },
                    "file": {
                        "type": "string",
                        "description": "MEMORY.md, memory_summary.md, raw_memories.md, \
                                        or rollout_summaries/<name>.md"
                    }
                },
                "required": ["project", "file"]
            }),
            json!({
                "type": "object",
                "properties": {
                    "project": { "type": "string" },
                    "file": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["project", "file", "content"]
            }),
        ),
    ]
}

/// Dispatch a `tools/call` for a memory tool. `None` only when `name` is not
/// a memory tool; handler failures are `isError: true` result envelopes.
pub fn handle_tool_call(name: &str, arguments: &Value) -> Option<Value> {
    match name {
        MEMORY_LIST_PROJECTS_TOOL => Some(list_projects_result(name)),
        MEMORY_LIST_TOOL => Some(list_result(name, arguments)),
        MEMORY_SEARCH_TOOL => Some(search_result(name, arguments)),
        MEMORY_READ_TOOL => Some(read_result(name, arguments)),
        _ => None,
    }
}

/// Resolve the memory root: `SLAB_MEMORIES_ROOT` override, else the app-home
/// default the memory pipeline itself uses.
pub fn memory_root() -> PathBuf {
    std::env::var_os(MEMORY_ROOT_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| slab_utils::app_home::app_home_dir().join("memories"))
}

fn list_projects_result(tool: &str) -> Value {
    let projects_root = memory_root().join(memory_fs::PROJECTS_DIR);
    let mut projects: Vec<String> = std::fs::read_dir(&projects_root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            entry.file_name().into_string().ok().filter(|name| !name.starts_with('.'))
        })
        .collect();
    projects.sort();

    tool_ok(
        tool,
        json!({
            "memory_root": memory_root().display().to_string(),
            "projects": projects,
        }),
    )
}

fn list_result(tool: &str, arguments: &Value) -> Value {
    let Some(project) = required_str(arguments, "project") else {
        return tool_error(tool, "missing required argument: project");
    };
    let project_root = memory_fs::project_memory_root(&memory_root(), project);
    let manifest = match recall::build_manifest(&project_root) {
        Ok(manifest) => manifest,
        Err(error) => return tool_error(tool, format!("memory manifest read failed: {error}")),
    };
    let entries: Vec<Value> = manifest.iter().map(manifest_entry_json).collect();

    tool_ok(tool, json!({ "project": project, "entries": entries }))
}

fn search_result(tool: &str, arguments: &Value) -> Value {
    let Some(project) = required_str(arguments, "project") else {
        return tool_error(tool, "missing required argument: project");
    };
    let Some(query) = required_str(arguments, "query") else {
        return tool_error(tool, "missing required argument: query");
    };
    let needle = query.to_lowercase();
    let project_root = memory_fs::project_memory_root(&memory_root(), project);
    let manifest = match recall::build_manifest(&project_root) {
        Ok(manifest) => manifest,
        Err(error) => return tool_error(tool, format!("memory manifest read failed: {error}")),
    };
    let matches: Vec<Value> = manifest
        .iter()
        .filter(|entry| {
            [&entry.filename, &entry.title, &entry.keywords, &entry.cwd]
                .iter()
                .any(|field| field.to_lowercase().contains(&needle))
        })
        .map(manifest_entry_json)
        .collect();

    tool_ok(tool, json!({ "project": project, "query": query, "matches": matches }))
}

fn read_result(tool: &str, arguments: &Value) -> Value {
    let Some(project) = required_str(arguments, "project") else {
        return tool_error(tool, "missing required argument: project");
    };
    let Some(file) = required_str(arguments, "file") else {
        return tool_error(tool, "missing required argument: file");
    };

    let project_root = memory_fs::project_memory_root(&memory_root(), project);
    let Some(path) = resolve_whitelisted_path(&project_root, file) else {
        return tool_error(tool, format!("file is not in the memory read surface: {file}"));
    };
    match read_chrooted(&project_root, &path) {
        Ok(content) => {
            tool_ok(tool, json!({ "project": project, "file": file, "content": content }))
        }
        Err(message) => tool_error(tool, message),
    }
}

/// Whitelist + shape-validate the requested file. Returns the candidate path
/// (still unchecked against symlinks — the caller canonicalizes).
fn resolve_whitelisted_path(project_root: &Path, file: &str) -> Option<PathBuf> {
    if file == memory_fs::MEMORY_REGISTRY_FILE
        || file == MEMORY_SUMMARY_FILE
        || file == memory_fs::RAW_MEMORIES_FILE
    {
        return Some(project_root.join(file));
    }
    let name = file.strip_prefix("rollout_summaries/").and_then(recall::safe_summary_name)?;
    Some(project_root.join("rollout_summaries").join(name))
}

/// Read `path` only if it canonicalizes back inside `project_root` — the
/// symlink-escape defense (a link planted anywhere on the workspace side must
/// not exfiltrate files outside it). A missing file is an error surfaced to
/// the caller.
fn read_chrooted(project_root: &Path, path: &Path) -> Result<String, String> {
    let canonical_root = project_root
        .canonicalize()
        .map_err(|_| format!("memory project workspace unavailable: {}", project_root.display()))?;
    match path.canonicalize() {
        Ok(resolved) if resolved.starts_with(&canonical_root) => {}
        Ok(_) => {
            return Err(format!("memory file escapes the project workspace: {}", path.display()));
        }
        Err(error) => return Err(format!("memory file unreadable: {error}")),
    }
    std::fs::read_to_string(path).map_err(|error| format!("memory file unreadable: {error}"))
}

fn manifest_entry_json(entry: &RecallManifestEntry) -> Value {
    json!({
        "filename": entry.filename,
        "title": entry.title,
        "keywords": entry.keywords,
        "cwd": entry.cwd,
        "updated_at": entry.updated_at.map(|at| at.to_rfc3339()),
    })
}

fn required_str<'a>(arguments: &'a Value, key: &str) -> Option<&'a str> {
    arguments.get(key).and_then(Value::as_str).filter(|value| !value.is_empty())
}

fn tool_spec(name: &str, title: &str, description: &str, input: Value, output: Value) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": input,
        "outputSchema": output,
        "annotations": {
            "title": title,
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        },
        "_meta": memory_tool_meta(name),
    })
}

fn tool_ok(tool: &str, structured: Value) -> Value {
    let text = structured.to_string();
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": structured,
        "isError": false,
        "_meta": memory_tool_meta(tool),
    })
}

fn tool_error(tool: &str, message: impl Into<String>) -> Value {
    let message = message.into();
    json!({
        "content": [{ "type": "text", "text": message.clone() }],
        "structuredContent": { "error": message },
        "isError": true,
        "_meta": memory_tool_meta(tool),
    })
}

fn memory_tool_meta(tool: &str) -> Value {
    json!({
        "slab": {
            "source": META_SOURCE,
            "permission": MEMORY_PERMISSION,
            "audit": {
                "event": "slab.mcp.tool_call",
                "tool": tool
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Materialize a throwaway memory root with one project store, a
    /// manifest-bearing raw_memories.md, and one summary file.
    fn seeded_root(tag: &str) -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().expect("tempdir");
        let project = memory_fs::project_memory_root(root.path(), "proj-a");
        std::fs::create_dir_all(project.join("rollout_summaries")).expect("project dirs");
        std::fs::write(
            project.join(memory_fs::RAW_MEMORIES_FILE),
            "## Thread t1\nsummary_file: rollout_summaries/alpha.md\ndescription: Alpha notes \
             on networking\nkeywords: tcp, latency\ncwd: /repo\n\nbody text\n",
        )
        .expect("raw memories");
        std::fs::write(
            project.join("rollout_summaries").join("alpha.md"),
            "# Alpha summary\n\nnetworking notes\n",
        )
        .expect("summary");
        std::fs::write(project.join(MEMORY_SUMMARY_FILE), "v1\n## User Profile\n- e2e\n")
            .expect("summary head");
        std::fs::write(project.join(memory_fs::MEMORY_REGISTRY_FILE), "# MEMORY\n- alpha\n")
            .expect("registry");
        let _ = tag;
        (root, project)
    }

    fn with_root_env<F, T>(root: &Path, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        // Tests run multi-threaded; guard the process-global env swap. The
        // unsafe env-mutation contract (no concurrent access) holds: this
        // mutex serializes every SLAB_MEMORIES_ROOT access in this crate.
        static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        // SAFETY: the lock above serializes all env mutations for this var.
        unsafe {
            std::env::set_var(MEMORY_ROOT_ENV, root);
        }
        let result = f();
        // SAFETY: the lock is still held.
        unsafe {
            std::env::remove_var(MEMORY_ROOT_ENV);
        }
        result
    }

    #[test]
    fn lists_projects_and_manifest_entries() {
        let (root, _project) = seeded_root("list");

        with_root_env(root.path(), || {
            let listed = handle_tool_call(MEMORY_LIST_PROJECTS_TOOL, &json!({}))
                .expect("list projects result");
            assert_eq!(listed["isError"], false);
            assert_eq!(listed["structuredContent"]["projects"], json!(["proj-a"]));

            let manifest = handle_tool_call(MEMORY_LIST_TOOL, &json!({ "project": "proj-a" }))
                .expect("list result");
            let entries = &manifest["structuredContent"]["entries"];
            assert_eq!(entries.as_array().expect("entries").len(), 1);
            assert_eq!(entries[0]["filename"], "rollout_summaries/alpha.md");
            assert_eq!(entries[0]["title"], "Alpha notes on networking");
            assert_eq!(entries[0]["keywords"], "tcp, latency");

            // Unknown project is an empty manifest, not an error.
            let empty = handle_tool_call(MEMORY_LIST_TOOL, &json!({ "project": "missing" }))
                .expect("list result");
            assert_eq!(empty["isError"], false);
            assert_eq!(empty["structuredContent"]["entries"], json!([]));
        });
    }

    #[test]
    fn searches_manifest_fields_case_insensitively() {
        let (root, _project) = seeded_root("search");

        with_root_env(root.path(), || {
            let hit = handle_tool_call(
                MEMORY_SEARCH_TOOL,
                &json!({ "project": "proj-a", "query": "LATENCY" }),
            )
            .expect("search result");
            assert_eq!(hit["isError"], false);
            assert_eq!(hit["structuredContent"]["matches"].as_array().expect("matches").len(), 1);

            let miss = handle_tool_call(
                MEMORY_SEARCH_TOOL,
                &json!({ "project": "proj-a", "query": "gpu" }),
            )
            .expect("search result");
            assert_eq!(miss["structuredContent"]["matches"], json!([]));

            let missing_query =
                handle_tool_call(MEMORY_SEARCH_TOOL, &json!({ "project": "proj-a" }))
                    .expect("search result");
            assert_eq!(missing_query["isError"], true);
        });
    }

    #[test]
    fn reads_whitelisted_files() {
        let (root, _project) = seeded_root("read");

        with_root_env(root.path(), || {
            for file in [
                memory_fs::MEMORY_REGISTRY_FILE,
                MEMORY_SUMMARY_FILE,
                memory_fs::RAW_MEMORIES_FILE,
                "rollout_summaries/alpha.md",
            ] {
                let read = handle_tool_call(
                    MEMORY_READ_TOOL,
                    &json!({ "project": "proj-a", "file": file }),
                )
                .expect("read result");
                assert_eq!(read["isError"], false, "{file} must be readable");
                assert!(
                    !read["structuredContent"]["content"].as_str().expect("content").is_empty()
                );
            }

            let missing = handle_tool_call(
                MEMORY_READ_TOOL,
                &json!({ "project": "proj-a", "file": "rollout_summaries/absent.md" }),
            )
            .expect("read result");
            assert_eq!(missing["isError"], true);
        });
    }

    #[test]
    fn read_rejects_traversal_and_non_whitelisted_files() {
        let (root, _project) = seeded_root("traversal");

        with_root_env(root.path(), || {
            for file in [
                "../proj-a/MEMORY.md",
                "rollout_summaries/../MEMORY.md",
                "rollout_summaries/../../etc/passwd",
                ".hidden",
                "rollout_summaries/.git",
                "phase2_workspace_diff.md",
                "C:/Windows/system32/config",
                "",
            ] {
                let read = handle_tool_call(
                    MEMORY_READ_TOOL,
                    &json!({ "project": "proj-a", "file": file }),
                )
                .expect("read result");
                assert_eq!(read["isError"], true, "file {file:?} must be rejected");
            }

            // A traversal-bearing project key re-sanitizes to a different
            // (missing) project — empty read error, never an escape.
            let escape = handle_tool_call(
                MEMORY_READ_TOOL,
                &json!({ "project": "../../etc", "file": "MEMORY.md" }),
            )
            .expect("read result");
            assert_eq!(escape["isError"], true);
        });
    }

    #[test]
    fn read_rejects_symlinked_summary_escape() {
        let (root, _project) = seeded_root("symlink");
        let outside = tempfile::tempdir().expect("outside tempdir");
        std::fs::write(outside.path().join("secret.md"), "secret\n").expect("secret");
        let link =
            memory_fs::project_memory_root(root.path(), "proj-a").join("rollout_summaries/evil.md");
        let symlink_result = {
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(outside.path().join("secret.md"), &link)
            }
            #[cfg(windows)]
            {
                std::os::windows::fs::symlink_file(outside.path().join("secret.md"), &link)
            }
        };
        if symlink_result.is_err() {
            // Symlink creation needs privileges on some hosts; skip silently.
            return;
        }

        with_root_env(root.path(), || {
            let read = handle_tool_call(
                MEMORY_READ_TOOL,
                &json!({ "project": "proj-a", "file": "rollout_summaries/evil.md" }),
            )
            .expect("read result");
            assert_eq!(read["isError"], true, "symlink escape must be rejected");
            let content = read["structuredContent"]["error"].as_str().expect("error");
            assert!(content.contains("escapes"), "unexpected error message: {content}");
        });
    }

    #[test]
    fn unknown_tool_name_returns_none() {
        assert!(handle_tool_call("memory_delete", &json!({})).is_none());
        assert!(is_memory_tool(MEMORY_READ_TOOL));
        assert!(!is_memory_tool("slab_server_info"));
        assert_eq!(tool_specs().len(), tool_names().len());
        for spec in tool_specs() {
            assert_eq!(spec["annotations"]["readOnlyHint"], true);
        }
    }
}
