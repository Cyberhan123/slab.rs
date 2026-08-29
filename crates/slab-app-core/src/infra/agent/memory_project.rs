//! Per-project memory identity resolution.
//!
//! The memory workspace shards by project (`<memory_root>/projects/<key>/`).
//! A project is the CANONICAL GIT ROOT of the workspace — so all worktrees of
//! one repository share a single memory store (Claude memdir semantics) —
//! falling back to the canonical workspace root when the directory is not a
//! git repository, and to the `''` sentinel when no workspace is bound.
//!
//! Process execution stays in the host crate (the memories crate is
//! deliberately persistence/supervision free); the git invocation mirrors
//! `slab-agent-memories::git`'s `std::process::Command` pattern.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use slab_agent_memories::fs as memory_fs;

/// Resolve the project key for a workspace root.
///
/// `None` (no workspace bound) resolves to `''`, which the memory layer maps
/// to the `_global` store. Results are cached per input root — the git
/// subprocess costs single-digit milliseconds, but the read side resolves the
/// key on every agent start.
pub(crate) fn resolve_project_key(workspace_root: Option<&Path>) -> String {
    let Some(root) = workspace_root else {
        return String::new();
    };
    let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    project_key_cache()
        .lock()
        .expect("memory project key cache poisoned")
        .entry(canonical.clone())
        .or_insert_with(|| memory_fs::sanitize_project_key(&git_root_or_self(&canonical)))
        .clone()
}

fn git_root_or_self(root: &Path) -> String {
    let output =
        Command::new("git").arg("-C").arg(root).args(["rev-parse", "--show-toplevel"]).output();
    if let Ok(output) = output
        && output.status.success()
    {
        let toplevel = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !toplevel.is_empty() {
            return toplevel;
        }
    }
    // Not a git repository (or git unavailable): the directory itself is the
    // project identity.
    root.to_string_lossy().into_owned()
}

fn project_key_cache() -> &'static Mutex<HashMap<PathBuf, String>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// One-shot backfill routing pre-sharding DB rows to `project_key`.
///
/// Called only when `memory_fs::adopt_legacy_layout` actually moved the flat
/// legacy workspace into the current project's store — those rows were
/// extracted from that workspace, so they belong to its project.
pub(crate) async fn backfill_project_key(
    pool: &sqlx::SqlitePool,
    project_key: &str,
) -> Result<(), String> {
    let updated =
        sqlx::query("UPDATE agent_memory_phase1_outputs SET project_key=?1 WHERE project_key=''")
            .bind(project_key)
            .execute(pool)
            .await
            .map_err(|error| error.to_string())?;
    tracing::debug!(
        rows = updated.rows_affected(),
        project_key,
        "backfilled memory project keys after legacy adoption"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_available() -> bool {
        Command::new("git").arg("--version").output().is_ok_and(|output| output.status.success())
    }

    #[test]
    fn resolves_key_from_git_toplevel_when_git_present() {
        if !git_available() {
            return;
        }
        // A real git repo in a temp dir; resolving from a NESTED directory
        // must return the repository root, not the input directory.
        let root = tempfile::tempdir().expect("tempdir");
        let status = Command::new("git")
            .arg("-C")
            .arg(root.path())
            .args(["init"])
            .output()
            .expect("git init");
        assert!(status.status.success(), "git init failed");
        let nested = root.path().join("nested").join("deep");
        std::fs::create_dir_all(&nested).expect("nested");

        let key = resolve_project_key(Some(&nested));

        let expected = memory_fs::sanitize_project_key(
            &root.path().canonicalize().expect("canonical").to_string_lossy(),
        );
        assert_eq!(key, expected);
    }

    #[test]
    fn falls_back_to_canonical_root_without_git() {
        let root = tempfile::tempdir().expect("tempdir");
        // No git init — the directory itself is the identity.
        let key = resolve_project_key(Some(root.path()));
        let expected = memory_fs::sanitize_project_key(
            &root.path().canonicalize().expect("canonical").to_string_lossy(),
        );
        assert_eq!(key, expected);
        assert!(!key.is_empty());
    }

    #[test]
    fn none_workspace_resolves_to_empty_sentinel() {
        assert_eq!(resolve_project_key(None), "");
    }
}
