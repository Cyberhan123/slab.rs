//! Canonical Slab application-home paths.

use std::path::PathBuf;

pub const APP_ID: &str = "cn.cyberhan.slab";

pub fn app_home_dir() -> PathBuf {
    app_home_dir_from_roots(dirs::config_dir(), dirs::home_dir())
}

fn app_home_dir_from_roots(config_root: Option<PathBuf>, home_root: Option<PathBuf>) -> PathBuf {
    config_root.or(home_root).unwrap_or_else(|| PathBuf::from(".")).join(APP_ID)
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

pub fn plugins_dir() -> PathBuf {
    app_home_dir().join("plugins")
}

pub fn rules_dir() -> PathBuf {
    app_home_dir().join("rules")
}

pub fn outputs_dir() -> PathBuf {
    app_home_dir().join("outputs")
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
    fn derived_paths_stay_under_app_home() {
        let home = app_home_dir();

        for path in [
            settings_path(),
            database_path(),
            logs_dir(),
            server_log_file(),
            runtime_log_dir(),
            runtime_ipc_dir(),
            models_dir(),
            sessions_dir(),
            plugins_dir(),
            rules_dir(),
            outputs_dir(),
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
