//! slab-server – entry point.
//!
//! Startup order:
//! 1. Parse configuration from environment variables.
//! 2. Initialise structured tracing (JSON in production, pretty in dev).
//! 3. Open the SQLite database and run pending migrations.
//! 4. Initialise the slab-core AI runtime.
//! 5. Start the IPC listener in a background task.
//! 6. Build the Axum router and start the HTTP server.

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
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| cfg.log_level.parse().unwrap_or_default()),
        )
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
        store: Arc::new(store),
    });

    // ── 6. IPC listener ────────────────────────────────────────────────────────
    let ipc_path  = cfg.ipc_socket_path.clone();
    let ipc_state = Arc::clone(&state);
    tokio::spawn(async move {
        if let Err(e) = ipc::serve(ipc_path, ipc_state).await {
            warn!(error = %e, "IPC listener exited");
        }
    });

    // ── 7. HTTP server ─────────────────────────────────────────────────────────
    let app      = routes::build(Arc::clone(&state));
    let addr: SocketAddr = cfg.bind_address.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "HTTP server listening");
    axum::serve(listener, app).await?;

    Ok(())
}
