//! `AGENTS.md` discovery.
//!
//! Reads the workspace `AGENTS.md` and the global app-home `AGENTS.md`.
//! Unlike `SKILL.md`, these are plain markdown (no YAML frontmatter) and are
//! injected verbatim.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// A discovered `AGENTS.md`: its absolute path and full body.
#[derive(Debug, Clone, Serialize)]
pub struct AgentMdRecord {
    pub path: PathBuf,
    pub body: String,
}

/// Read workspace then global `AGENTS.md`. Missing files are silently skipped;
/// an empty result means neither was present.
pub fn scan_agents_md(
    workspace_root: Option<&Path>,
    app_home_agents_md: &Path,
) -> Vec<AgentMdRecord> {
    let mut records = Vec::new();
    if let Some(root) = workspace_root {
        let path = root.join("AGENTS.md");
        if let Ok(body) = std::fs::read_to_string(&path) {
            records.push(AgentMdRecord { path, body });
        }
    }
    if let Ok(body) = std::fs::read_to_string(app_home_agents_md) {
        records.push(AgentMdRecord { path: app_home_agents_md.to_path_buf(), body });
    }
    records
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn reads_workspace_then_global() {
        let ws = TempDir::new().unwrap();
        let global = TempDir::new().unwrap();
        fs::write(ws.path().join("AGENTS.md"), "# ws\n").unwrap();
        fs::write(global.path().join("AGENTS.md"), "# global\n").unwrap();
        let records = scan_agents_md(Some(ws.path()), &global.path().join("AGENTS.md"));
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].body, "# ws\n");
        assert_eq!(records[1].body, "# global\n");
    }

    #[test]
    fn missing_files_yield_empty() {
        let ws = TempDir::new().unwrap();
        let records = scan_agents_md(Some(ws.path()), Path::new("/nope/AGENTS.md"));
        assert!(records.is_empty());
    }
}
