//! slab-server entry point.
//! Runs in supervisor mode by default.

mod config;
mod entities;
mod error;
mod grpc;
mod middleware;
mod routes;
mod schemas;
mod state;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use clap::Parser;
use tokio::process::{Child, Command as TokioCommand};
use tracing::{info, warn};

use crate::config::Config;
use crate::entities::{AnyStore, TaskStore};
use crate::state::{AppState, TaskManager};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "slab-server",
    version,
    about = "Slab supervisor and HTTP gateway"
)]
struct SupervisorArgs {
    #[arg(long, default_value = "127.0.0.1:3000")]
    gateway_bind: String,
    #[arg(long, default_value = "127.0.0.1:3001")]
    whisper_bind: String,
    #[arg(long, default_value = "127.0.0.1:3002")]
    llama_bind: String,
    #[arg(long, default_value = "127.0.0.1:3003")]
    diffusion_bind: String,
    #[arg(long, default_value_t = false)]
    include_diffusion: bool,
    #[arg(long = "database-url")]
    database_url: Option<String>,
    #[arg(long = "log")]
    log_level: Option<String>,
    #[arg(long = "log-json", action = clap::ArgAction::SetTrue)]
    log_json: bool,
    #[arg(long = "queue-capacity")]
    queue_capacity: Option<usize>,
    #[arg(long = "backend-capacity")]
    backend_capacity: Option<usize>,
    #[arg(long = "lib-dir")]
    lib_dir: Option<PathBuf>,
}

impl Default for SupervisorArgs {
    fn default() -> Self {
        Self {
            gateway_bind: "127.0.0.1:3000".to_owned(),
            whisper_bind: "127.0.0.1:3001".to_owned(),
            llama_bind: "127.0.0.1:3002".to_owned(),
            diffusion_bind: "127.0.0.1:3003".to_owned(),
            include_diffusion: false,
            database_url: None,
            log_level: None,
            log_json: false,
            queue_capacity: None,
            backend_capacity: None,
            lib_dir: None,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = SupervisorArgs::parse();
    let mut cfg = Config::from_env();

    if let Some(log_level) = &args.log_level {
        cfg.log_level = log_level.clone();
    }
    if args.log_json {
        cfg.log_json = true;
    }
    if !args.log_json && cfg.log_json {
        args.log_json = true;
    }
    if args.database_url.is_none() {
        args.database_url = Some(cfg.database_url.clone());
    }
    if args.log_level.is_none() {
        args.log_level = Some(cfg.log_level.clone());
    }
    if args.queue_capacity.is_none() {
        args.queue_capacity = Some(cfg.queue_capacity);
    }
    if args.backend_capacity.is_none() {
        args.backend_capacity = Some(cfg.backend_capacity);
    }
    if args.lib_dir.is_none() {
        args.lib_dir = cfg.lib_dir.clone();
    }

    init_tracing(&cfg.log_level, cfg.log_json);
    run_supervisor(args).await
}

fn init_tracing(log_level: &str, log_json: bool) {
    let env_filter = match tracing_subscriber::EnvFilter::try_from_default_env() {
        Ok(f) => f,
        Err(_) => match log_level.parse::<tracing_subscriber::EnvFilter>() {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "WARN: SLAB_LOG='{}' is not a valid tracing filter ({}); falling back to 'info'",
                    log_level, e
                );
                tracing_subscriber::EnvFilter::new("info")
            }
        },
    };

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(true);

    if log_json {
        subscriber.json().init();
    } else {
        subscriber.init();
    }
}

async fn run_gateway(cfg: Config) -> anyhow::Result<()> {
    info!(
        version = env!("CARGO_PKG_VERSION"),
        "slab-server gateway starting"
    );

    if let Err(e) = tokio::fs::create_dir_all(&cfg.session_state_dir).await {
        warn!(
            path = %cfg.session_state_dir,
            error = %e,
            "failed to create session state dir"
        );
    }

    let store = AnyStore::connect(&cfg.database_url).await?;
    info!(database_url = %cfg.database_url, "database ready");

    let state = Arc::new(AppState {
        config: Arc::new(cfg.clone()),
        store: Arc::new(store.clone()),
        task_manager: Arc::new(TaskManager::new()),
    });

    let app = routes::build_gateway(Arc::clone(&state));
    let addr: SocketAddr = cfg.bind_address.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "HTTP gateway listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    if let Err(e) = store.interrupt_running_tasks().await {
        warn!(
            error = %e,
            "failed to interrupt running tasks on shutdown"
        );
    }

    info!("slab-server gateway stopped");
    Ok(())
}

#[derive(Debug)]
struct ManagedChild {
    backend: String,
    bind_address: String,
    child: Child,
}

fn spawn_backend_child(
    runtime_exe: &Path,
    backend: &str,
    grpc_bind_address: &str,
    args: &SupervisorArgs,
) -> anyhow::Result<ManagedChild> {
    let mut cmd = TokioCommand::new(runtime_exe);
    cmd.arg("--enabled-backends")
        .arg(backend)
        .arg("--grpc-bind")
        .arg(grpc_bind_address)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::null());

    if let Some(v) = &args.lib_dir {
        cmd.arg("--lib-dir").arg(v);
    }
    if let Some(v) = args.queue_capacity {
        cmd.arg("--queue-capacity").arg(v.to_string());
    }
    if let Some(v) = args.backend_capacity {
        cmd.arg("--backend-capacity").arg(v.to_string());
    }
    if let Some(v) = &args.log_level {
        cmd.arg("--log").arg(v);
    }
    if args.log_json {
        cmd.arg("--log-json");
    }

    let child = cmd.spawn().with_context(|| {
        format!(
            "failed to spawn slab-runtime child '{}' from {}",
            backend,
            runtime_exe.display()
        )
    })?;
    info!(
        backend = backend,
        bind_address = grpc_bind_address,
        pid = ?child.id(),
        "spawned backend child process"
    );
    Ok(ManagedChild {
        backend: backend.to_string(),
        bind_address: grpc_bind_address.to_string(),
        child,
    })
}

async fn shutdown_children(children: &mut [ManagedChild]) {
    for managed in children.iter_mut() {
        match managed.child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) => {
                if let Err(e) = managed.child.start_kill() {
                    warn!(
                        backend = %managed.backend,
                        bind_address = %managed.bind_address,
                        error = %e,
                        "failed to signal child kill"
                    );
                }
            }
            Err(e) => {
                warn!(
                    backend = %managed.backend,
                    bind_address = %managed.bind_address,
                    error = %e,
                    "failed to query child status before shutdown"
                );
            }
        }
    }

    for managed in children.iter_mut() {
        match managed.child.try_wait() {
            Ok(Some(status)) => {
                info!(
                    backend = %managed.backend,
                    bind_address = %managed.bind_address,
                    status = %status,
                    "child process exited"
                );
            }
            Ok(None) => {
                match tokio::time::timeout(Duration::from_secs(5), managed.child.wait()).await {
                    Ok(Ok(status)) => {
                        info!(
                            backend = %managed.backend,
                            bind_address = %managed.bind_address,
                            status = %status,
                            "child process exited after kill"
                        );
                    }
                    Ok(Err(e)) => {
                        warn!(
                            backend = %managed.backend,
                            bind_address = %managed.bind_address,
                            error = %e,
                            "failed while waiting child exit"
                        );
                    }
                    Err(_) => {
                        warn!(
                            backend = %managed.backend,
                            bind_address = %managed.bind_address,
                            "timed out waiting for child exit"
                        );
                    }
                }
            }
            Err(e) => {
                warn!(
                    backend = %managed.backend,
                    bind_address = %managed.bind_address,
                    error = %e,
                    "failed to query child status"
                );
            }
        }
    }
}

fn resolve_runtime_exe(server_exe: &Path) -> anyhow::Result<PathBuf> {
    let parent = server_exe
        .parent()
        .ok_or_else(|| anyhow!("failed to resolve server executable parent directory"))?;
    let server_name = server_exe
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("server executable name is not valid UTF-8"))?;
    let ext = if cfg!(windows) { ".exe" } else { "" };

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(rest) = server_name.strip_prefix("slab-server-") {
        candidates.push(parent.join(format!("slab-runtime-{rest}")));
    }
    candidates.push(parent.join(format!("slab-runtime{ext}")));

    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if cfg!(windows) {
                if name.starts_with("slab-runtime-") && name.ends_with(".exe") {
                    candidates.push(path);
                }
            } else if name.starts_with("slab-runtime-") {
                candidates.push(path);
            }
        }
    }

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    anyhow::bail!(
        "slab-runtime executable not found near {}. Build and bundle slab-runtime sidecar first.",
        server_exe.display()
    );
}

async fn run_supervisor(args: SupervisorArgs) -> anyhow::Result<()> {
    info!("slab-server supervisor starting");
    let server_exe =
        std::env::current_exe().context("failed to resolve current executable path")?;
    let runtime_exe = resolve_runtime_exe(&server_exe)?;
    let mut children = Vec::new();

    children.push(spawn_backend_child(
        &runtime_exe,
        "whisper",
        &args.whisper_bind,
        &args,
    )?);
    children.push(spawn_backend_child(
        &runtime_exe,
        "llama",
        &args.llama_bind,
        &args,
    )?);
    if args.include_diffusion {
        children.push(spawn_backend_child(
            &runtime_exe,
            "diffusion",
            &args.diffusion_bind,
            &args,
        )?);
    }

    info!(
        child_count = children.len(),
        gateway_bind = %args.gateway_bind,
        "supervisor started backend children and is booting HTTP gateway"
    );

    let mut gateway_cfg = Config::from_env();
    if let Some(v) = &args.database_url {
        gateway_cfg.database_url = v.clone();
    }
    if let Some(v) = &args.log_level {
        gateway_cfg.log_level = v.clone();
    }
    if args.log_json {
        gateway_cfg.log_json = true;
    }
    gateway_cfg.bind_address = args.gateway_bind.clone();
    gateway_cfg.transport_mode = "http".to_string();
    gateway_cfg.whisper_grpc_endpoint = Some(args.whisper_bind.clone());
    gateway_cfg.llama_grpc_endpoint = Some(args.llama_bind.clone());
    gateway_cfg.diffusion_grpc_endpoint = if args.include_diffusion {
        Some(args.diffusion_bind.clone())
    } else {
        None
    };

    let mut gateway_join = tokio::spawn(async move { run_gateway(gateway_cfg).await });

    let mut result = Ok(());
    loop {
        tokio::select! {
            _ = shutdown_signal() => {
                info!("supervisor received shutdown signal");
                break;
            }
            gateway_res = &mut gateway_join => {
                result = match gateway_res {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(e)) => Err(e),
                    Err(e) => Err(anyhow!("gateway task join error: {e}")),
                };
                break;
            }
            _ = tokio::time::sleep(Duration::from_millis(500)) => {
                for managed in children.iter_mut() {
                    match managed.child.try_wait() {
                        Ok(Some(status)) => {
                            result = Err(anyhow!(
                                "backend child '{}' on {} exited unexpectedly with status {}",
                                managed.backend,
                                managed.bind_address,
                                status
                            ));
                            break;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            result = Err(anyhow!(
                                "failed to query backend child '{}' on {}: {}",
                                managed.backend,
                                managed.bind_address,
                                e
                            ));
                            break;
                        }
                    }
                }
                if result.is_err() {
                    break;
                }
            }
        }
    }

    if !gateway_join.is_finished() {
        gateway_join.abort();
        let _ = gateway_join.await;
    }

    shutdown_children(&mut children).await;
    info!("supervisor stopped");
    result
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

    tokio::select! {
        _ = ctrl_c   => {}
        _ = terminate => {}
    }
    info!("shutdown signal received; starting graceful shutdown");
}
