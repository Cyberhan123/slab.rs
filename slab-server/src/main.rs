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
mod entities;
mod error;
mod ipc;
mod middleware;
mod routes;
mod schemas;
mod state;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use tracing::{info, warn};

use crate::config::Config;
use crate::entities::{AnyStore, TaskStore};

use crate::state::{AppState, TaskManager};

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
    let store = AnyStore::connect(&cfg.database_url).await?;
    info!(database_url = %cfg.database_url, "database ready");

    // ── 4. slab-core AI runtime ────────────────────────────────────────────────

    let base_lib_path = cfg.lib_dir.as_deref().unwrap_or(&Path::new("./resources/libs"));
    let llama_lib_dir = base_lib_path.join("llama");
    let whisper_lib_dir = base_lib_path.join("whisper");
    let diffusion_lib_dir = base_lib_path.join("diffusion");
    info!(
        llama_lib_dir = %llama_lib_dir.display(),
        whisper_lib_dir = %whisper_lib_dir.display(),
        diffusion_lib_dir = %diffusion_lib_dir.display(),
        "initialising slab-core with library paths"
    );

    slab_core::api::init(slab_core::api::Config {
        queue_capacity: cfg.queue_capacity,
        backend_capacity: cfg.backend_capacity,
        llama_lib_dir: Some(llama_lib_dir.to_path_buf()),
        whisper_lib_dir: Some(whisper_lib_dir.to_path_buf()),
        diffusion_lib_dir: Some(diffusion_lib_dir.to_path_buf()),
    })?;

    info!("slab-core runtime initialised");

    // ── 5. Session state directory ─────────────────────────────────────────────
    if let Err(e) = tokio::fs::create_dir_all(&cfg.session_state_dir).await {
        warn!(path = %cfg.session_state_dir, error = %e, "failed to create session state dir");
    }

    // ── 6. Shared application state ────────────────────────────────────────────
    let state = Arc::new(AppState {
        config: Arc::new(cfg.clone()),
        store: Arc::new(store.clone()),
        task_manager: Arc::new(TaskManager::new()),
    });

    // ── 7. IPC listener ────────────────────────────────────────────────────────
    let transport = cfg.transport_mode.as_str();
    if transport == "ipc" || transport == "both" {
        let ipc_path = cfg.ipc_socket_path.clone();
        let ipc_state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(e) = ipc::serve(ipc_path, ipc_state).await {
                warn!(error = %e, "IPC listener exited");
            }
        });
    }

    // ── 8. HTTP server with graceful shutdown ──────────────────────────────────
    if transport == "http" || transport == "both" {
        let app = routes::build(Arc::clone(&state));
        let addr: SocketAddr = cfg.bind_address.parse()?;
        let listener = tokio::net::TcpListener::bind(addr).await?;
        info!(%addr, "HTTP server listening");
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await?;
    } else {
        // IPC-only mode: wait for shutdown signal without binding TCP.
        shutdown_signal().await;
    }

    // Interrupt any tasks that were running when the server stopped.
    if let Err(e) = store.interrupt_running_tasks().await {
        warn!(error = %e, "failed to interrupt running tasks on shutdown");
    }

    // Clean up the IPC socket file so the next startup can bind immediately.
    if transport == "ipc" || transport == "both" {
        let ipc_cleanup = cfg.ipc_socket_path.clone();
        if let Err(e) = tokio::fs::remove_file(&ipc_cleanup).await {
            warn!(
                path = %ipc_cleanup,
                error = %e,
                "failed to remove IPC socket file on shutdown (may not exist)"
            );
        }
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
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => warn!(error = %e, "failed to install SIGTERM handler"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    // let stdin_closed = async {
    //     use tokio::io::AsyncReadExt;
    //     let mut stdin = tokio::io::stdin();
    //     let mut buf = [0u8; 1];
    //     let _ = stdin.read(&mut buf).await;
    // };

    tokio::select! {
        _ = ctrl_c   => {}
        _ = terminate => {}
        // _ = stdin_closed => {}
    }
    info!("shutdown signal received; starting graceful shutdown");
}
