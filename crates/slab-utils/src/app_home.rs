//! Canonical Slab application-home paths.

use std::path::PathBuf;

pub const APP_ID: &str = "cn.cyberhan.slab";

pub fn app_home_dir() -> PathBuf {
    app_home_dir_from_roots(dirs::config_dir(), dirs::home_dir())
}

fn app_home_dir_from_roots(config_root: Option<PathBuf>, home_root: Option<PathBuf>) -> PathBuf {
    config_root.or(home_root).unwrap_or_else(|| PathBuf::from(".")).join(APP_ID)
}

/// User-visible root for global (workspace-less) session artifacts:
/// `<Documents>/slab`. Sessions created without a project workspace get a
/// per-session directory under this root (recorded in
/// `chat_sessions.state_path`), so their file-tool output lands somewhere the
/// user can find instead of the process cwd. Falls back to the home directory
/// when the OS reports no Documents folder.
///
/// `SLAB_GLOBAL_SESSIONS_DIR` overrides the root (the test harnesses point it
/// at a tempdir so spawned servers never touch the real Documents tree).
pub fn global_sessions_root() -> PathBuf {
    if let Some(root) =
        std::env::var_os("SLAB_GLOBAL_SESSIONS_DIR").filter(|value| !value.is_empty())
    {
        return PathBuf::from(root);
    }
    global_sessions_root_from_documents(dirs::document_dir())
}

fn global_sessions_root_from_documents(documents_root: Option<PathBuf>) -> PathBuf {
    documents_root.or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from(".")).join("slab")
}

pub fn settings_path() -> PathBuf {
    app_home_dir().join("settings.json")
}

pub fn database_path() -> PathBuf {
    app_home_dir().join("slab.db")
}

pub fn logs_dir() -> PathBuf {
    app_home_dir().join("logs")
}

pub fn server_log_file() -> PathBuf {
    logs_dir().join("slab-server.log")
}

/// Sandbox audit log (`<app_home>/logs/slab-sandbox.log`). Structured JSON lines
/// recording sandbox spawn/provision decisions for troubleshooting.
pub fn sandbox_log_file() -> PathBuf {
    logs_dir().join("slab-sandbox.log")
}

pub fn runtime_log_dir() -> PathBuf {
    logs_dir().join("runtime")
}

pub fn runtime_ipc_dir() -> PathBuf {
    app_home_dir().join("ipc")
}

pub fn models_dir() -> PathBuf {
    app_home_dir().join("models")
}

pub fn sessions_dir() -> PathBuf {
    app_home_dir().join("sessions")
}

/// Global skills directory (`<app_home>/skills`). Mirror of the workspace
/// `.agents/skills` tree, scanned by `slab-agent-context`.
pub fn skills_dir() -> PathBuf {
    app_home_dir().join("skills")
}

/// Global `AGENTS.md` path (`<app_home>/AGENTS.md`).
pub fn agents_md_path() -> PathBuf {
    app_home_dir().join("AGENTS.md")
}

pub fn plugins_dir() -> PathBuf {
    app_home_dir().join("plugins")
}

pub fn rules_dir() -> PathBuf {
    app_home_dir().join("rules")
}

pub fn outputs_dir() -> PathBuf {
    app_home_dir().join("outputs")
}

/// Durable plans directory (`<app_home>/plans`). Plans authored in plan mode
/// (the plan agent's `plan` / `update_plan` tools) are persisted here as JSON
/// so they survive process restarts.
pub fn plans_dir() -> PathBuf {
    app_home_dir().join("plans")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_home_uses_app_id() {
        assert_eq!(app_home_dir().file_name().and_then(|name| name.to_str()), Some(APP_ID));
    }

    #[test]
    fn app_home_uses_injected_config_root() {
        let config_root = PathBuf::from("C:/Users/example/AppData/Roaming");

        assert_eq!(
            app_home_dir_from_roots(
                Some(config_root.clone()),
                Some(PathBuf::from("C:/Users/example"))
            ),
            config_root.join(APP_ID)
        );
    }

    #[test]
    fn global_sessions_root_uses_documents_then_home() {
        assert_eq!(
            global_sessions_root_from_documents(Some(PathBuf::from("C:/Users/example/Documents"))),
            PathBuf::from("C:/Users/example/Documents/slab")
        );
        assert_eq!(
            global_sessions_root_from_documents(None),
            dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join("slab")
        );
    }

    #[test]
    fn derived_paths_stay_under_app_home() {
        let home = app_home_dir();

        for path in [
            settings_path(),
            database_path(),
            logs_dir(),
            server_log_file(),
            sandbox_log_file(),
            runtime_log_dir(),
            runtime_ipc_dir(),
            models_dir(),
            sessions_dir(),
            skills_dir(),
            agents_md_path(),
            plugins_dir(),
            rules_dir(),
            outputs_dir(),
            plans_dir(),
        ] {
            assert!(
                path.starts_with(&home),
                "{} should stay under {}",
                path.display(),
                home.display()
            );
        }
    }
}
