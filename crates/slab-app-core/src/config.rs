//! Server configuration, loaded from environment variables at startup.

use dirs_next::config_dir;
use std::path::{Path, PathBuf};

/// Runtime configuration for slab-server.
///
/// Every field has a sensible default so the server works out-of-the-box
/// without any environment variables set.
#[derive(Debug, Clone)]
pub struct Config {
    /// TCP address to bind (default: `"0.0.0.0:3000"`).
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
    /// When `None` (default), all origins are allowed (`*`).
    ///
    /// **Security note:** The wildcard default is convenient for development
    /// but should be restricted to trusted origins in production.
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
            bind_address: env_or("SLAB_BIND", "localhost:3000"),
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
            settings_path,
            model_config_dir,
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

pub fn sqlite_url_for_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let prefix = if normalized.starts_with('/') { "sqlite://" } else { "sqlite:///" };
    format!("{prefix}{normalized}?mode=rwc")
}
