//! Server configuration, loaded from environment variables at startup.

use slab_types::{DESKTOP_API_BIND, sqlite_url_for_path};
use slab_utils::app_home;
use std::path::{Path, PathBuf};

use crate::{PluginJsRuntimeTransport, PluginPythonRuntimeTransport, SettingsDocument};

/// Supplies environment values to [`Config::from_env_source`] so tests can
/// exercise config parsing without mutating process-global state.
trait EnvSource {
    fn var(&self, key: &str) -> Option<String>;
}

struct ProcessEnv;

impl EnvSource for ProcessEnv {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

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
    /// By default this resolves to an absolute SQLite file under the Slab app
    /// home (for example `%AppData%\cn.cyberhan.slab\slab.db` on Windows).
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
    /// When `None`, unauthenticated management access is allowed only on
    /// loopback bind addresses.
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

    /// Optional Candle llama backend gRPC endpoint used by HTTP gateway mode.
    pub candle_llama_grpc_endpoint: Option<String>,

    /// Optional Candle whisper backend gRPC endpoint used by HTTP gateway mode.
    pub candle_whisper_grpc_endpoint: Option<String>,

    /// Optional Candle diffusion backend gRPC endpoint used by HTTP gateway mode.
    pub candle_diffusion_grpc_endpoint: Option<String>,

    /// Directory containing the llama, whisper, and diffusion shared libraries.
    pub lib_dir: Option<std::path::PathBuf>,

    /// Directory where chat session state files are stored.
    pub session_state_dir: String,

    /// Absolute path of the user-managed settings values file.
    pub settings_path: PathBuf,

    /// Optional workspace-local settings overlay file.
    pub settings_overlay_path: Option<PathBuf>,

    /// Optional workspace root used by workspace services and agent tools.
    pub workspace_root: Option<PathBuf>,

    /// Directory containing persisted model config JSON files.
    ///
    /// Files in this directory are scanned during startup and upserted into the
    /// unified `models` table so the catalog can be initialized from bundled or
    /// user-managed config files.
    pub model_config_dir: PathBuf,

    /// Root directory containing installed runtime plugins.
    pub plugins_dir: PathBuf,

    /// Directory containing shell execution `.rule` files.
    pub exec_rules_dir: PathBuf,

    /// Transport used when app-core talks to the JS plugin sidecar runtime.
    pub plugin_js_runtime_transport: PluginJsRuntimeTransport,

    /// Transport used when app-core talks to the Python plugin sidecar runtime.
    pub plugin_python_runtime_transport: PluginPythonRuntimeTransport,
}

pub type AppConfig = Config;

impl Config {
    /// Build [`Config`] from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        Self::from_env_source(&ProcessEnv)
    }

    fn from_env_source(source: &impl EnvSource) -> Self {
        let settings_path = source
            .var("SLAB_SETTINGS_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(default_settings_path);
        let model_config_dir = source
            .var("SLAB_MODEL_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| default_model_config_dir_for_settings_path(&settings_path));

        Self {
            bind_address: env_or(source, "SLAB_BIND", DESKTOP_API_BIND),
            database_url: source.var("SLAB_DATABASE_URL").unwrap_or_else(default_database_url),
            log_level: env_or(source, "SLAB_LOG", "info"),
            log_json: parse_trueish_env(source, "SLAB_LOG_JSON"),
            log_file: source.var("SLAB_LOG_FILE").map(PathBuf::from),
            cloud_http_trace: parse_trueish_env(source, "SLAB_CLOUD_HTTP_TRACE"),
            queue_capacity: parse_env(source, "SLAB_QUEUE_CAPACITY", 64),
            backend_capacity: parse_env(source, "SLAB_BACKEND_CAPACITY", 4),
            enable_swagger: source
                .var("SLAB_ENABLE_SWAGGER")
                .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
                .unwrap_or(true),
            cors_allowed_origins: source.var("SLAB_CORS_ORIGINS"),
            admin_api_token: source.var("SLAB_ADMIN_TOKEN"),
            transport_mode: env_or(source, "SLAB_TRANSPORT", "http"),
            llama_grpc_endpoint: source.var("SLAB_LLAMA_GRPC_ENDPOINT"),
            whisper_grpc_endpoint: source.var("SLAB_WHISPER_GRPC_ENDPOINT"),
            diffusion_grpc_endpoint: source.var("SLAB_DIFFUSION_GRPC_ENDPOINT"),
            candle_llama_grpc_endpoint: source.var("SLAB_CANDLE_LLAMA_GRPC_ENDPOINT"),
            candle_whisper_grpc_endpoint: source.var("SLAB_CANDLE_WHISPER_GRPC_ENDPOINT"),
            candle_diffusion_grpc_endpoint: source.var("SLAB_CANDLE_DIFFUSION_GRPC_ENDPOINT"),
            lib_dir: source.var("SLAB_LIB_DIR").map(PathBuf::from),
            session_state_dir: source
                .var("SLAB_SESSION_STATE_DIR")
                .unwrap_or_else(|| default_session_state_dir().to_string_lossy().into_owned()),
            settings_path: settings_path.clone(),
            settings_overlay_path: source.var("SLAB_SETTINGS_OVERLAY_PATH").map(PathBuf::from),
            workspace_root: source.var("SLAB_WORKSPACE_ROOT").map(PathBuf::from),
            model_config_dir,
            plugins_dir: plugin_install_dir_from_settings(&settings_path)
                .unwrap_or_else(|| default_plugin_install_dir_for_settings_path(&settings_path)),
            exec_rules_dir: source
                .var("SLAB_EXEC_RULES_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(default_exec_rules_dir),
            plugin_js_runtime_transport: plugin_js_runtime_transport_from_settings(&settings_path)
                .unwrap_or_default(),
            plugin_python_runtime_transport: plugin_python_runtime_transport_from_settings(
                &settings_path,
            )
            .unwrap_or_default(),
        }
    }
}

// ── private helpers ──────────────────────────────────────────────────────────

fn env_or(source: &impl EnvSource, key: &str, default: &str) -> String {
    source.var(key).unwrap_or_else(|| default.to_owned())
}

fn parse_env<T: std::str::FromStr>(source: &impl EnvSource, key: &str, default: T) -> T {
    source.var(key).and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn parse_trueish_env(source: &impl EnvSource, key: &str) -> bool {
    source.var(key).map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false)
}

pub fn default_app_dir() -> PathBuf {
    app_home::app_home_dir()
}

pub fn default_settings_path() -> PathBuf {
    app_home::settings_path()
}

pub fn default_model_config_dir() -> PathBuf {
    default_model_config_dir_for_settings_path(&default_settings_path())
}

pub fn default_database_path() -> PathBuf {
    app_home::database_path()
}

pub fn default_database_url() -> String {
    sqlite_url_for_path(&default_database_path())
}

pub fn default_session_state_dir() -> PathBuf {
    app_home::sessions_dir()
}

pub fn default_plugins_dir() -> PathBuf {
    default_plugin_install_dir_for_settings_path(&default_settings_path())
}

pub fn default_exec_rules_dir() -> PathBuf {
    default_exec_rules_dir_for_settings_path(&default_settings_path())
}

fn plugin_install_dir_from_settings(settings_path: &Path) -> Option<PathBuf> {
    let document = settings_document_from_path(settings_path)?;
    document
        .plugin
        .install_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn plugin_js_runtime_transport_from_settings(
    settings_path: &Path,
) -> Option<PluginJsRuntimeTransport> {
    let document = settings_document_from_path(settings_path)?;
    Some(document.plugin.js_runtime_transport)
}

fn plugin_python_runtime_transport_from_settings(
    settings_path: &Path,
) -> Option<PluginPythonRuntimeTransport> {
    let document = settings_document_from_path(settings_path)?;
    Some(document.plugin.python_runtime_transport)
}

fn settings_document_from_path(settings_path: &Path) -> Option<SettingsDocument> {
    let raw = std::fs::read_to_string(settings_path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn default_plugin_install_dir_for_settings_path(settings_path: &Path) -> PathBuf {
    let _ = settings_path;
    app_home::plugins_dir()
}

pub fn default_exec_rules_dir_for_settings_path(settings_path: &Path) -> PathBuf {
    let _ = settings_path;
    app_home::rules_dir()
}

pub fn default_runtime_log_dir() -> PathBuf {
    app_home::runtime_log_dir()
}

pub fn default_runtime_ipc_dir() -> PathBuf {
    app_home::runtime_ipc_dir()
}

pub fn default_model_config_dir_for_settings_path(settings_path: &Path) -> PathBuf {
    let _ = settings_path;
    app_home::models_dir()
}

pub fn default_output_dir_for_settings_path(settings_path: &Path) -> PathBuf {
    let _ = settings_path;
    app_home::outputs_dir()
}

#[cfg(test)]
mod tests {
    use super::{
        Config, EnvSource, default_database_path, default_exec_rules_dir,
        default_exec_rules_dir_for_settings_path, default_model_config_dir,
        default_plugin_install_dir_for_settings_path, default_plugins_dir, default_runtime_ipc_dir,
        default_runtime_log_dir, default_session_state_dir, default_settings_path,
    };
    use slab_types::DESKTOP_API_BIND;
    use slab_utils::app_home;
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    impl EnvSource for HashMap<String, String> {
        fn var(&self, key: &str) -> Option<String> {
            self.get(key).cloned()
        }
    }

    fn env_vars<const N: usize>(entries: [(&str, &str); N]) -> HashMap<String, String> {
        entries.into_iter().map(|(key, value)| (key.to_owned(), value.to_owned())).collect()
    }

    fn temp_settings_fixture() -> (TempDir, PathBuf) {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let settings_path = temp_dir.path().join("settings.json");
        (temp_dir, settings_path)
    }

    fn write_json(path: &Path, value: serde_json::Value) {
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(path, serde_json::to_string_pretty(&value).expect("serialize")).expect("write");
    }

    #[test]
    fn from_env_uses_desktop_api_bind_by_default() {
        let env = HashMap::<String, String>::new();
        let config = Config::from_env_source(&env);
        assert_eq!(config.bind_address, DESKTOP_API_BIND);
    }

    #[test]
    fn default_paths_use_app_home() {
        let app_home = app_home::app_home_dir();

        for path in [
            default_settings_path(),
            default_database_path(),
            default_model_config_dir(),
            default_session_state_dir(),
            default_plugins_dir(),
            default_exec_rules_dir(),
            default_runtime_log_dir(),
            default_runtime_ipc_dir(),
        ] {
            assert!(
                path.starts_with(&app_home),
                "{} should stay under {}",
                path.display(),
                app_home.display()
            );
        }
    }

    #[test]
    fn from_env_defaults_plugins_dir_to_app_home() {
        let (_temp_dir, settings_path) = temp_settings_fixture();
        let settings_path = settings_path.to_string_lossy().into_owned();
        let env = env_vars([("SLAB_SETTINGS_PATH", settings_path.as_str())]);
        let config = Config::from_env_source(&env);

        assert_eq!(
            config.plugins_dir,
            default_plugin_install_dir_for_settings_path(Path::new(&settings_path))
        );
    }

    #[test]
    fn from_env_ignores_slab_plugins_dir_override() {
        let (_temp_dir, settings_path) = temp_settings_fixture();
        let ignored_plugins_dir = settings_path.parent().expect("parent").join("ignored-plugins");
        let settings_path = settings_path.to_string_lossy().into_owned();
        let ignored_plugins_dir = ignored_plugins_dir.to_string_lossy().into_owned();
        let env = env_vars([
            ("SLAB_SETTINGS_PATH", settings_path.as_str()),
            ("SLAB_PLUGINS_DIR", ignored_plugins_dir.as_str()),
        ]);
        let config = Config::from_env_source(&env);

        assert_eq!(
            config.plugins_dir,
            default_plugin_install_dir_for_settings_path(Path::new(&settings_path))
        );
    }

    #[test]
    fn from_env_uses_settings_plugin_install_dir_when_present() {
        let (_temp_dir, settings_path) = temp_settings_fixture();
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
        let settings_path = settings_path.to_string_lossy().into_owned();
        let env = env_vars([("SLAB_SETTINGS_PATH", settings_path.as_str())]);
        let config = Config::from_env_source(&env);

        assert_eq!(config.plugins_dir, configured_plugins_dir);
    }

    #[test]
    fn exec_rules_dir_defaults_to_app_home() {
        let settings_path = PathBuf::from("C:/Slab/settings.json");

        assert_eq!(default_exec_rules_dir_for_settings_path(&settings_path), app_home::rules_dir());
    }

    #[test]
    fn from_env_uses_exec_rules_dir_override() {
        let rules_dir = PathBuf::from("D:/Slab/rules");
        let rules_dir = rules_dir.to_string_lossy().into_owned();
        let env = env_vars([("SLAB_EXEC_RULES_DIR", rules_dir.as_str())]);
        let config = Config::from_env_source(&env);

        assert_eq!(config.exec_rules_dir, PathBuf::from(rules_dir));
    }

    #[test]
    fn from_env_reads_network_logging_and_auth_overrides() {
        let env = env_vars([
            ("SLAB_DATABASE_URL", "sqlite:///tmp/slab-test.db"),
            ("SLAB_LOG_JSON", "TRUE"),
            ("SLAB_CLOUD_HTTP_TRACE", "1"),
            ("SLAB_ENABLE_SWAGGER", "false"),
            ("SLAB_QUEUE_CAPACITY", "0"),
            ("SLAB_BACKEND_CAPACITY", "12"),
            ("SLAB_ADMIN_TOKEN", "test-admin-token"),
            ("SLAB_CORS_ORIGINS", "https://app.example.com,https://admin.example.com"),
            ("SLAB_TRANSPORT", "ipc"),
        ]);
        let config = Config::from_env_source(&env);

        assert_eq!(config.database_url, "sqlite:///tmp/slab-test.db");
        assert!(config.log_json);
        assert!(config.cloud_http_trace);
        assert!(!config.enable_swagger);
        assert_eq!(config.queue_capacity, 0);
        assert_eq!(config.backend_capacity, 12);
        assert_eq!(config.admin_api_token.as_deref(), Some("test-admin-token"));
        assert_eq!(
            config.cors_allowed_origins.as_deref(),
            Some("https://app.example.com,https://admin.example.com")
        );
        assert_eq!(config.transport_mode, "ipc");
    }

    #[test]
    fn from_env_falls_back_for_invalid_capacity_and_unrecognized_boolean_values() {
        let env = env_vars([
            ("SLAB_LOG_JSON", "yes"),
            ("SLAB_CLOUD_HTTP_TRACE", "on"),
            ("SLAB_ENABLE_SWAGGER", "0"),
            ("SLAB_QUEUE_CAPACITY", "not-a-number"),
            ("SLAB_BACKEND_CAPACITY", "999999999999999999999999999999999999"),
        ]);
        let config = Config::from_env_source(&env);

        assert!(!config.log_json);
        assert!(!config.cloud_http_trace);
        assert!(!config.enable_swagger);
        assert_eq!(config.queue_capacity, 64);
        assert_eq!(config.backend_capacity, 4);
    }
}
