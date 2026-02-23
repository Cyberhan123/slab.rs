//! slab-server – entry point.
//!
//! Startup order:
//! 1. Parse configuration from environment variables.
//! 2. Initialise structured tracing (JSON in production, pretty in dev).
//! 3. Open the SQLite database and run pending migrations.
//! 4. Initialise the slab-core AI runtime.
//! 5. Start the IPC listener in a background task.
//! 6. Build the Axum router and start the HTTP server with graceful shutdown.

mod config;
mod db;
mod error;
mod ipc;
mod middleware;
mod models;
mod routes;
mod state;

use std::net::SocketAddr;
use std::sync::Arc;

use tracing::{info, warn};

use crate::config::Config;
use crate::db::sqlite::SqliteStore;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── 1. Configuration ───────────────────────────────────────────────────────
    let cfg = Config::from_env();

    // ── 2. Tracing ─────────────────────────────────────────────────────────────
    // Build the log-level filter, warning loudly if the configured value is
    // not a valid tracing filter expression.
    let env_filter = match tracing_subscriber::EnvFilter::try_from_default_env() {
        Ok(f) => f,
        Err(_) => match cfg.log_level.parse::<tracing_subscriber::EnvFilter>() {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "WARN: SLAB_LOG='{}' is not a valid tracing filter ({}); \
                     falling back to 'info'",
                    cfg.log_level, e
                );
                tracing_subscriber::EnvFilter::new("info")
            }
        },
    };

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(true);

    if cfg.log_json {
        subscriber.json().init();
    } else {
        subscriber.init();
    }

    info!(version = env!("CARGO_PKG_VERSION"), "slab-server starting");

    // ── 3. Database ────────────────────────────────────────────────────────────
    let store = SqliteStore::connect(&cfg.database_url).await?;
    info!(database_url = %cfg.database_url, "database ready");

    // ── 4. slab-core AI runtime ────────────────────────────────────────────────
    slab_core::api::init(slab_core::api::Config {
        queue_capacity:   cfg.queue_capacity,
        backend_capacity: cfg.backend_capacity,
    })?;
    info!("slab-core runtime initialised");

    // ── 5. Shared application state ────────────────────────────────────────────
    let state = Arc::new(AppState {
        config: Arc::new(cfg.clone()),
        store:  Arc::new(store),
    });

    // ── 6. IPC listener ────────────────────────────────────────────────────────
    let ipc_path  = cfg.ipc_socket_path.clone();
    let ipc_state = Arc::clone(&state);
    tokio::spawn(async move {
        if let Err(e) = ipc::serve(ipc_path, ipc_state).await {
            warn!(error = %e, "IPC listener exited");
        }
    });

    // ── 7. HTTP server with graceful shutdown ──────────────────────────────────
    let app      = routes::build(Arc::clone(&state));
    let addr: SocketAddr = cfg.bind_address.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "HTTP server listening");

    let ipc_cleanup = cfg.ipc_socket_path.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Clean up the IPC socket file so the next startup can bind immediately.
    if let Err(e) = tokio::fs::remove_file(&ipc_cleanup).await {
        warn!(
            path = %ipc_cleanup,
            error = %e,
            "failed to remove IPC socket file on shutdown (may not exist)"
        );
    }

    info!("slab-server stopped");
    Ok(())
}

/// Returns a future that resolves when SIGINT (Ctrl-C) or SIGTERM is received.
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            warn!(error = %e, "failed to install CTRL+C signal handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut s) => { s.recv().await; }
            Err(e)    => warn!(error = %e, "failed to install SIGTERM handler"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c   => {}
        _ = terminate => {}
    }

    info!("shutdown signal received; starting graceful shutdown");
}
