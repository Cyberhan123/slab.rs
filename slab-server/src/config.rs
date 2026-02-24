//! Server configuration, loaded from environment variables at startup.

/// Runtime configuration for slab-server.
///
/// Every field has a sensible default so the server works out-of-the-box
/// without any environment variables set.
#[derive(Debug, Clone)]
pub struct Config {
    /// TCP address to bind (default: `"0.0.0.0:3000"`).
    pub bind_address: String,

    /// SQLite (or other) database URL (default: `"sqlite://slab.db"`).
    ///
    /// The path in a `sqlite://` URL is relative to the **current working
    /// directory** of the server process at startup.  For predictable
    /// behaviour in production, use an absolute path, e.g.
    /// `SLAB_DATABASE_URL=sqlite:///var/lib/slab/slab.db`.
    ///
    /// Supports any sqlx-compatible connection string – swap the scheme to
    /// migrate to Postgres (`postgres://…`) or MySQL (`mysql://…`).
    pub database_url: String,

    /// Filesystem path for the IPC Unix-domain socket.
    ///
    /// **Security note:** The default `/tmp/slab-server.sock` is world-
    /// readable on most systems.  In production, set this to a path inside a
    /// directory with restricted permissions (e.g. `/var/run/slab/server.sock`
    /// owned by the service user) so that only authorised local processes can
    /// connect.
    pub ipc_socket_path: String,

    /// `tracing` filter string, e.g. `"info"` or `"debug,tower_http=warn"`.
    pub log_level: String,

    /// When `true`, emit log records as newline-delimited JSON.
    pub log_json: bool,

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

    /// Optional bearer token required for admin endpoints
    /// (`/api/models/…`).  Set `SLAB_ADMIN_TOKEN=<secret>` to require
    /// an `Authorization: Bearer <secret>` header on those routes.
    /// When `None`, admin endpoints are unauthenticated.
    pub admin_api_token: Option<String>,

    /// Transport mode: `"http"`, `"ipc"`, or `"both"` (default: `"http"`).
    pub transport_mode: String,

    /// Directory containing the llama shared library.
    pub llama_lib_dir: Option<String>,

    /// Directory containing the whisper shared library.
    pub whisper_lib_dir: Option<String>,

    /// Directory containing the stable-diffusion shared library.
    pub diffusion_lib_dir: Option<String>,

    /// Directory where chat session state files are stored.
    pub session_state_dir: String,
}

impl Config {
    /// Build [`Config`] from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        Self {
            bind_address: env_or("SLAB_BIND", "0.0.0.0:3000"),
            database_url: env_or("SLAB_DATABASE_URL", "sqlite://slab.db?mode=rwc"),
            ipc_socket_path: env_or("SLAB_IPC_SOCKET", "/tmp/slab-server.sock"),
            log_level: env_or("SLAB_LOG", "info"),
            log_json: std::env::var("SLAB_LOG_JSON")
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
            llama_lib_dir: std::env::var("SLAB_LLAMA_LIB_DIR").ok(),
            whisper_lib_dir: std::env::var("SLAB_WHISPER_LIB_DIR").ok(),
            diffusion_lib_dir: std::env::var("SLAB_DIFFUSION_LIB_DIR").ok(),
            session_state_dir: env_or("SLAB_SESSION_STATE_DIR", "/tmp/slab-sessions"),
        }
    }
}

// ── private helpers ──────────────────────────────────────────────────────────

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
