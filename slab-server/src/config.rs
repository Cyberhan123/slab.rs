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
    /// Supports any sqlx-compatible connection string – swap the scheme to
    /// migrate to Postgres (`postgres://…`) or MySQL (`mysql://…`).
    pub database_url: String,

    /// Filesystem path for the IPC Unix-domain socket.
    /// On Windows this is treated as a named-pipe name.
    pub ipc_socket_path: String,

    /// `tracing` filter string, e.g. `"info"` or `"debug,tower_http=warn"`.
    pub log_level: String,

    /// When `true`, emit log records as newline-delimited JSON.
    pub log_json: bool,

    /// Orchestrator submission-queue capacity (passed to slab-core).
    pub queue_capacity: usize,

    /// Maximum concurrent in-flight requests per AI backend.
    pub backend_capacity: usize,
}

impl Config {
    /// Build [`Config`] from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        Self {
            bind_address: env_or("SLAB_BIND", "0.0.0.0:3000"),
            database_url: env_or("SLAB_DATABASE_URL", "sqlite://slab.db"),
            ipc_socket_path: env_or("SLAB_IPC_SOCKET", "/tmp/slab-server.sock"),
            log_level: env_or("SLAB_LOG", "info"),
            log_json: std::env::var("SLAB_LOG_JSON")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            queue_capacity: parse_env("SLAB_QUEUE_CAPACITY", 64),
            backend_capacity: parse_env("SLAB_BACKEND_CAPACITY", 4),
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
