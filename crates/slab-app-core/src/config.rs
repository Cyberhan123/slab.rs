//! Server configuration, loaded from environment variables at startup.

use dirs_next::config_dir;
use slab_types::DESKTOP_API_BIND;
use std::path::{Path, PathBuf};

/// Runtime configuration for slab-server.
///
/// Every field has a sensible default so the server works out-of-the-box
/// without any environment variables set.
#[derive(Debug, Clone)]
pub struct Config {
    /// TCP address to bind (default: `"127.0.0.1:3000"`).
    pub bind_address: String,

    /// SQLite (or other) database URL.
    ///
    /// By default this resolves to an absolute SQLite file in the user's Slab
    /// config directory (for example `%AppData%\Slab\slab.db` on Windows).
    /// Override it with `SLAB_DATABASE_URL` to point elsewhere.
    ///
    /// Supports any sqlx-compatible connection string – swap the scheme to
    /// migrate to Postgres (`postgres://…`) or MySQL (`mysql://…`).
    pub database_url: String,

    /// `tracing` filter string, e.g. `"info"` or `"debug,tower_http=warn"`.
    pub log_level: String,

    /// When `true`, emit log records as newline-delimited JSON.
    pub log_json: bool,

    /// Optional path to a file that receives appended server logs.
    pub log_file: Option<PathBuf>,

    /// When `true`, log redacted outbound cloud chat HTTP request/response data.
    ///
    /// This is intended for short-lived debugging sessions only because it can
    /// include full prompt/response bodies in the server logs.
    pub cloud_http_trace: bool,

    /// Orchestrator submission-queue capacity (passed to slab-core).
    pub queue_capacity: usize,

    /// Maximum concurrent in-flight requests per AI backend.
    pub backend_capacity: usize,

    /// When `true`, serve the Swagger UI at `/swagger-ui` and the OpenAPI spec
    /// at `/api-docs/openapi.json`.  Set `SLAB_ENABLE_SWAGGER=false` to
    /// disable in production if you don't want the API structure exposed.
    pub enable_swagger: bool,

    /// Comma-separated list of allowed CORS origins, e.g.
    /// `"https://app.example.com,https://admin.example.com"`.
    /// When `None`, slab-server falls back to the built-in desktop/dev allowlist.
    pub cors_allowed_origins: Option<String>,

    /// Optional bearer token required for management endpoints (`/v1/settings*`, `/v1/backends/*`).
    ///
    /// Set `SLAB_ADMIN_TOKEN=<secret>` to require an
    /// `Authorization: Bearer <secret>` header on those routes.
    /// When `None`, admin endpoints are unauthenticated.
    pub admin_api_token: Option<String>,

    /// Runtime transport mode between slab-server and slab-runtime:
    /// `"http"` or `"ipc"` (default: `"http"`).
    pub transport_mode: String,

    /// Optional llama backend gRPC endpoint used by HTTP gateway mode.
    pub llama_grpc_endpoint: Option<String>,

    /// Optional whisper backend gRPC endpoint used by HTTP gateway mode.
    pub whisper_grpc_endpoint: Option<String>,

    /// Optional diffusion backend gRPC endpoint used by HTTP gateway mode.
    pub diffusion_grpc_endpoint: Option<String>,

    /// Directory containing the llama, whisper, and diffusion shared libraries.
    pub lib_dir: Option<std::path::PathBuf>,

    /// Directory where chat session state files are stored.
    pub session_state_dir: String,

    /// Absolute path of the user-managed settings values file.
    pub settings_path: PathBuf,

    /// Directory containing persisted model config JSON files.
    ///
    /// Files in this directory are scanned during startup and upserted into the
    /// unified `models` table so the catalog can be initialized from bundled or
    /// user-managed config files.
    pub model_config_dir: PathBuf,

    /// Root directory containing installed runtime plugins.
    pub plugins_dir: PathBuf,
}

impl Config {
    /// Build [`Config`] from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        let settings_path = std::env::var("SLAB_SETTINGS_PATH")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(default_settings_path);
        let model_config_dir = std::env::var("SLAB_MODEL_CONFIG_DIR")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| default_model_config_dir_for_settings_path(&settings_path));

        Self {
            bind_address: env_or("SLAB_BIND", DESKTOP_API_BIND),
            database_url: std::env::var("SLAB_DATABASE_URL")
                .unwrap_or_else(|_| default_database_url()),
            log_level: env_or("SLAB_LOG", "info"),
            log_json: std::env::var("SLAB_LOG_JSON")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            log_file: std::env::var("SLAB_LOG_FILE").ok().map(PathBuf::from),
            cloud_http_trace: std::env::var("SLAB_CLOUD_HTTP_TRACE")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            queue_capacity: parse_env("SLAB_QUEUE_CAPACITY", 64),
            backend_capacity: parse_env("SLAB_BACKEND_CAPACITY", 4),
            enable_swagger: std::env::var("SLAB_ENABLE_SWAGGER")
                .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
                .unwrap_or(true),
            cors_allowed_origins: std::env::var("SLAB_CORS_ORIGINS").ok(),
            admin_api_token: std::env::var("SLAB_ADMIN_TOKEN").ok(),
            transport_mode: env_or("SLAB_TRANSPORT", "http"),
            llama_grpc_endpoint: std::env::var("SLAB_LLAMA_GRPC_ENDPOINT").ok(),
            whisper_grpc_endpoint: std::env::var("SLAB_WHISPER_GRPC_ENDPOINT").ok(),
            diffusion_grpc_endpoint: std::env::var("SLAB_DIFFUSION_GRPC_ENDPOINT").ok(),
            lib_dir: std::env::var("SLAB_LIB_DIR").ok().map(PathBuf::from),
            session_state_dir: std::env::var("SLAB_SESSION_STATE_DIR")
                .unwrap_or_else(|_| default_session_state_dir().to_string_lossy().into_owned()),
            settings_path: settings_path.clone(),
            model_config_dir,
            plugins_dir: plugin_install_dir_from_settings(&settings_path)
                .unwrap_or_else(|| default_plugin_install_dir_for_settings_path(&settings_path)),
        }
    }
}

// ── private helpers ──────────────────────────────────────────────────────────

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

pub fn default_app_dir() -> PathBuf {
    config_dir().unwrap_or_else(|| PathBuf::from(".")).join("Slab")
}

pub fn default_settings_path() -> PathBuf {
    default_app_dir().join("settings.json")
}

pub fn default_model_config_dir() -> PathBuf {
    default_model_config_dir_for_settings_path(&default_settings_path())
}

pub fn default_database_path() -> PathBuf {
    default_app_dir().join("slab.db")
}

pub fn default_database_url() -> String {
    sqlite_url_for_path(&default_database_path())
}

pub fn default_session_state_dir() -> PathBuf {
    default_app_dir().join("sessions")
}

pub fn default_plugins_dir() -> PathBuf {
    default_plugin_install_dir_for_settings_path(&default_settings_path())
}

fn plugin_install_dir_from_settings(settings_path: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(settings_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value
        .pointer("/plugin/install_dir")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn default_plugin_install_dir_for_settings_path(settings_path: &Path) -> PathBuf {
    settings_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("plugins")
}

pub fn default_runtime_log_dir() -> PathBuf {
    default_app_dir().join("logs").join("runtime")
}

pub fn default_runtime_ipc_dir() -> PathBuf {
    default_app_dir().join("ipc")
}

pub fn default_model_config_dir_for_settings_path(settings_path: &Path) -> PathBuf {
    settings_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("models")
}

pub fn default_output_dir_for_settings_path(settings_path: &Path) -> PathBuf {
    settings_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("outputs")
}

pub fn sqlite_url_for_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let prefix = if normalized.starts_with('/') { "sqlite://" } else { "sqlite:///" };
    format!("{prefix}{normalized}?mode=rwc")
}

#[cfg(test)]
mod tests {
    use super::{Config, default_plugin_install_dir_for_settings_path};
    use slab_types::DESKTOP_API_BIND;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        key: &'static str,
        value: Option<String>,
    }

    impl EnvGuard {
        fn capture(key: &'static str) -> Self {
            Self { key, value: std::env::var(key).ok() }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.value {
                Some(value) => unsafe { std::env::set_var(self.key, value) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }

    fn temp_settings_path() -> PathBuf {
        std::env::temp_dir()
            .join(format!("slab-config-test-{}", uuid::Uuid::new_v4()))
            .join("settings.json")
    }

    fn write_json(path: &Path, value: serde_json::Value) {
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(path, serde_json::to_string_pretty(&value).expect("serialize")).expect("write");
    }

    #[test]
    fn from_env_uses_desktop_api_bind_by_default() {
        let _lock = env_lock().lock().unwrap();
        let _bind = EnvGuard::capture("SLAB_BIND");
        let _settings = EnvGuard::capture("SLAB_SETTINGS_PATH");
        let _model_config = EnvGuard::capture("SLAB_MODEL_CONFIG_DIR");
        let _plugins = EnvGuard::capture("SLAB_PLUGINS_DIR");

        unsafe {
            std::env::remove_var("SLAB_BIND");
            std::env::remove_var("SLAB_SETTINGS_PATH");
            std::env::remove_var("SLAB_MODEL_CONFIG_DIR");
            std::env::remove_var("SLAB_PLUGINS_DIR");
        }

        let config = Config::from_env();
        assert_eq!(config.bind_address, DESKTOP_API_BIND);
    }

    #[test]
    fn from_env_defaults_plugins_dir_next_to_settings_path() {
        let _lock = env_lock().lock().unwrap();
        let _settings = EnvGuard::capture("SLAB_SETTINGS_PATH");
        let _model_config = EnvGuard::capture("SLAB_MODEL_CONFIG_DIR");
        let _plugins = EnvGuard::capture("SLAB_PLUGINS_DIR");
        let settings_path = temp_settings_path();

        unsafe {
            std::env::set_var("SLAB_SETTINGS_PATH", &settings_path);
            std::env::remove_var("SLAB_MODEL_CONFIG_DIR");
            std::env::remove_var("SLAB_PLUGINS_DIR");
        }

        let config = Config::from_env();

        assert_eq!(
            config.plugins_dir,
            default_plugin_install_dir_for_settings_path(&settings_path)
        );
    }

    #[test]
    fn from_env_ignores_slab_plugins_dir_override() {
        let _lock = env_lock().lock().unwrap();
        let _settings = EnvGuard::capture("SLAB_SETTINGS_PATH");
        let _plugins = EnvGuard::capture("SLAB_PLUGINS_DIR");
        let settings_path = temp_settings_path();
        let ignored_plugins_dir = settings_path.parent().expect("parent").join("ignored-plugins");

        unsafe {
            std::env::set_var("SLAB_SETTINGS_PATH", &settings_path);
            std::env::set_var("SLAB_PLUGINS_DIR", &ignored_plugins_dir);
        }

        let config = Config::from_env();

        assert_eq!(
            config.plugins_dir,
            default_plugin_install_dir_for_settings_path(&settings_path)
        );
    }

    #[test]
    fn from_env_uses_settings_plugin_install_dir_when_present() {
        let _lock = env_lock().lock().unwrap();
        let _settings = EnvGuard::capture("SLAB_SETTINGS_PATH");
        let _plugins = EnvGuard::capture("SLAB_PLUGINS_DIR");
        let settings_path = temp_settings_path();
        let configured_plugins_dir =
            settings_path.parent().expect("parent").join("configured-plugins");
        write_json(
            &settings_path,
            serde_json::json!({
                "plugin": {
                    "install_dir": configured_plugins_dir.to_string_lossy()
                }
            }),
        );

        unsafe {
            std::env::set_var("SLAB_SETTINGS_PATH", &settings_path);
            std::env::remove_var("SLAB_PLUGINS_DIR");
        }

        let config = Config::from_env();

        assert_eq!(config.plugins_dir, configured_plugins_dir);
        let _ = fs::remove_dir_all(settings_path.parent().expect("parent"));
    }
}
