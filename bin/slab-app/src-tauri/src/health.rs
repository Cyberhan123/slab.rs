//! First-run installer health check + `.slab` workspace bootstrap (INFRA-10).
//!
//! `run_first_run_health_check` probes that the canonical app-home directories
//! are writable so a fresh install can persist settings / logs / models /
//! plugins / sessions. `bootstrap_slab_directory` idempotently creates the
//! `.slab` workspace scaffold (settings template + gitignore) for a workspace
//! root. Both are host-only Tauri commands.

use std::path::{Path, PathBuf};

use serde::Serialize;
use slab_utils::app_home;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    pub name: String,
    pub path: String,
    pub ok: bool,
    pub guidance: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckResult {
    pub overall_ok: bool,
    pub checks: Vec<HealthCheck>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapResult {
    pub slab_dir: String,
    pub settings_created: bool,
    pub gitignore_created: bool,
}

/// Probe the canonical app-home directories and report which are writable.
#[tauri::command]
pub fn run_first_run_health_check() -> HealthCheckResult {
    let probes = [
        ("app_home", app_home::app_home_dir()),
        ("logs", app_home::logs_dir()),
        ("models", app_home::models_dir()),
        ("plugins", app_home::plugins_dir()),
        ("sessions", app_home::sessions_dir()),
    ];

    let checks = probes
        .into_iter()
        .map(|(name, path)| {
            let ok = probe_dir_writable(&path);
            HealthCheck {
                name: name.to_string(),
                path: path.display().to_string(),
                ok,
                guidance: if ok {
                    None
                } else {
                    Some(format!(
                        "Slab cannot write to {}. Check directory permissions for the Slab app home.",
                        path.display()
                    ))
                },
            }
        })
        .collect::<Vec<_>>();

    let overall_ok = checks.iter().all(|check| check.ok);
    HealthCheckResult { overall_ok, checks }
}

/// Idempotently create the `.slab` scaffold (settings template + gitignore) for
/// a workspace root. Existing files are never overwritten.
#[tauri::command]
pub fn bootstrap_slab_directory(root: String) -> Result<BootstrapResult, String> {
    let root_path = PathBuf::from(root.trim());
    bootstrap_slab_directory_at(&root_path)
}

/// Pure core of [`bootstrap_slab_directory`] so the idempotency logic is testable
/// without a Tauri runtime.
pub(crate) fn bootstrap_slab_directory_at(root: &Path) -> Result<BootstrapResult, String> {
    if !root.is_dir() {
        return Err(format!("workspace root {} is not a directory", root.display()));
    }

    let slab_dir = root.join(".slab");
    std::fs::create_dir_all(&slab_dir)
        .map_err(|error| format!("failed to create {}: {error}", slab_dir.display()))?;

    let settings_path = slab_dir.join("settings.json");
    let settings_created = !settings_path.exists();
    if settings_created {
        std::fs::write(&settings_path, DEFAULT_SLAB_SETTINGS)
            .map_err(|error| format!("failed to write {}: {error}", settings_path.display()))?;
    }

    let gitignore_path = slab_dir.join(".gitignore");
    let gitignore_created = !gitignore_path.exists();
    if gitignore_created {
        std::fs::write(&gitignore_path, SLAB_GITIGNORE)
            .map_err(|error| format!("failed to write {}: {error}", gitignore_path.display()))?;
    }

    Ok(BootstrapResult {
        slab_dir: slab_dir.display().to_string(),
        settings_created,
        gitignore_created,
    })
}

/// Probe that a directory can be created and a small file written, read, and
/// removed. Pure (no app-home assumptions) so it is testable with temp dirs.
pub(crate) fn probe_dir_writable(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".slab-health-probe");
    std::fs::write(&probe, b"ok").is_ok()
        && std::fs::read(&probe).is_ok()
        && std::fs::remove_file(&probe).is_ok()
}

const DEFAULT_SLAB_SETTINGS: &str = "{\n}\n";

const SLAB_GITIGNORE: &str = "# Local Slab workspace state\n";

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{bootstrap_slab_directory_at, probe_dir_writable};

    #[test]
    fn probe_dir_writable_true_for_writable_dir_and_cleans_up() {
        let temp = tempfile::tempdir().expect("tempdir");

        assert!(probe_dir_writable(temp.path()));

        // The probe file must not be left behind.
        assert!(!temp.path().join(".slab-health-probe").exists());
    }

    #[test]
    fn probe_dir_writable_false_when_parent_is_a_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        // Create a file, then try to treat a path *under* it as a directory.
        let file = temp.path().join("not-a-dir");
        fs::write(&file, b"x").unwrap();

        assert!(!probe_dir_writable(&file.join("child")));
    }

    #[test]
    fn bootstrap_creates_scaffold_and_is_idempotent() {
        let temp = tempfile::tempdir().expect("tempdir");

        let first = bootstrap_slab_directory_at(temp.path()).expect("first bootstrap");
        assert!(first.settings_created);
        assert!(first.gitignore_created);
        assert!(temp.path().join(".slab").join("settings.json").is_file());
        assert!(temp.path().join(".slab").join(".gitignore").is_file());

        // Second run must not overwrite existing files.
        let second = bootstrap_slab_directory_at(temp.path()).expect("second bootstrap");
        assert!(!second.settings_created);
        assert!(!second.gitignore_created);
    }

    #[test]
    fn bootstrap_rejects_missing_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let missing = temp.path().join("missing");

        let error = bootstrap_slab_directory_at(&missing).expect_err("missing root rejected");
        assert!(error.contains("not a directory"));
    }
}
