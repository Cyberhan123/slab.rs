//! slab-server entry point.
//!
//! `serve` runs a single server instance.
//! `supervisor` spawns isolated `serve` subprocesses (one backend per process).

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
use clap::{Args, Parser, Subcommand};
use tokio::process::{Child, Command as TokioCommand};
use tracing::{info, warn};

use crate::config::Config;
use crate::entities::{AnyStore, TaskStore};
use crate::state::{AppState, TaskManager};

#[derive(Parser, Debug)]
#[command(name = "slab-server", version, about = "Slab server runtime")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run a single slab-server instance in this process.
    Serve(ServeArgs),
    /// Run backend-isolated slab-server subprocesses and supervise them.
    Supervisor(SupervisorArgs),
}

#[derive(Args, Debug, Default, Clone)]
struct ServeArgs {
    #[arg(long = "bind")]
    bind_address: Option<String>,
    #[arg(long = "database-url")]
    database_url: Option<String>,
    #[arg(long = "ipc-socket")]
    ipc_socket_path: Option<String>,
    #[arg(long = "log")]
    log_level: Option<String>,
    #[arg(long = "log-json", action = clap::ArgAction::SetTrue)]
    log_json: bool,
    #[arg(long = "queue-capacity")]
    queue_capacity: Option<usize>,
    #[arg(long = "backend-capacity")]
    backend_capacity: Option<usize>,
    #[arg(long = "transport")]
    transport_mode: Option<String>,
    #[arg(long = "grpc-bind")]
    grpc_bind_address: Option<String>,
    #[arg(long = "llama-grpc-endpoint")]
    llama_grpc_endpoint: Option<String>,
    #[arg(long = "whisper-grpc-endpoint")]
    whisper_grpc_endpoint: Option<String>,
    #[arg(long = "diffusion-grpc-endpoint")]
    diffusion_grpc_endpoint: Option<String>,
    #[arg(long = "lib-dir")]
    lib_dir: Option<PathBuf>,
    #[arg(long = "enabled-backends")]
    enabled_backends: Option<String>,
    #[arg(long = "session-state-dir")]
    session_state_dir: Option<String>,
}

#[derive(Args, Debug, Clone)]
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
    #[arg(long = "transport")]
    transport_mode: Option<String>,
    #[arg(long = "lib-dir")]
    lib_dir: Option<PathBuf>,
    #[arg(long = "session-state-base", default_value = "./tmp")]
    session_state_base: PathBuf,
}

#[derive(Debug, Clone, Copy)]
struct EnabledBackends {
    llama: bool,
    whisper: bool,
    diffusion: bool,
}

impl EnabledBackends {
    fn all() -> Self {
        Self {
            llama: true,
            whisper: true,
            diffusion: true,
        }
    }
}

fn parse_enabled_backends(raw: Option<&str>) -> anyhow::Result<EnabledBackends> {
    let Some(raw) = raw.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(EnabledBackends::all());
    };

    let mut enabled = EnabledBackends {
        llama: false,
        whisper: false,
        diffusion: false,
    };
    let mut unknown = Vec::new();

    for token in raw.split(',').map(str::trim).filter(|v| !v.is_empty()) {
        match token.to_ascii_lowercase().as_str() {
            "llama" | "ggml.llama" => enabled.llama = true,
            "whisper" | "ggml.whisper" => enabled.whisper = true,
            "diffusion" | "ggml.diffusion" => enabled.diffusion = true,
            other => unknown.push(other.to_string()),
        }
    }

    if !unknown.is_empty() {
        anyhow::bail!(
            "invalid SLAB_ENABLED_BACKENDS entries: {}. Supported values: llama, whisper, diffusion",
            unknown.join(", ")
        );
    }

    if !enabled.llama && !enabled.whisper && !enabled.diffusion {
        anyhow::bail!(
            "SLAB_ENABLED_BACKENDS must contain at least one backend: llama, whisper, diffusion"
        );
    }

    Ok(enabled)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Serve(ServeArgs::default())) {
        Command::Serve(args) => {
            let mut cfg = Config::from_env();
            apply_serve_overrides(&mut cfg, &args);
            init_tracing(&cfg.log_level, cfg.log_json);
            run_server(cfg).await
        }
        Command::Supervisor(args) => {
            let mut cfg = Config::from_env();
            if let Some(log_level) = &args.log_level {
                cfg.log_level = log_level.clone();
            }
            if args.log_json {
                cfg.log_json = true;
            }
            init_tracing(&cfg.log_level, cfg.log_json);
            run_supervisor(args).await
        }
    }
}

fn apply_serve_overrides(cfg: &mut Config, args: &ServeArgs) {
    if let Some(v) = &args.bind_address {
        cfg.bind_address = v.clone();
    }
    if let Some(v) = &args.database_url {
        cfg.database_url = v.clone();
    }
    if let Some(v) = &args.ipc_socket_path {
        cfg.ipc_socket_path = v.clone();
    }
    if let Some(v) = &args.log_level {
        cfg.log_level = v.clone();
    }
    if args.log_json {
        cfg.log_json = true;
    }
    if let Some(v) = args.queue_capacity {
        cfg.queue_capacity = v;
    }
    if let Some(v) = args.backend_capacity {
        cfg.backend_capacity = v;
    }
    if let Some(v) = &args.transport_mode {
        cfg.transport_mode = v.clone();
    }
    if let Some(v) = &args.grpc_bind_address {
        cfg.grpc_bind_address = v.clone();
    }
    if let Some(v) = &args.llama_grpc_endpoint {
        cfg.llama_grpc_endpoint = Some(v.clone());
    }
    if let Some(v) = &args.whisper_grpc_endpoint {
        cfg.whisper_grpc_endpoint = Some(v.clone());
    }
    if let Some(v) = &args.diffusion_grpc_endpoint {
        cfg.diffusion_grpc_endpoint = Some(v.clone());
    }
    if let Some(v) = &args.lib_dir {
        cfg.lib_dir = Some(v.clone());
    }
    if let Some(v) = &args.enabled_backends {
        cfg.enabled_backends = Some(v.clone());
    }
    if let Some(v) = &args.session_state_dir {
        cfg.session_state_dir = v.clone();
    }
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

async fn run_server(cfg: Config) -> anyhow::Result<()> {
    info!(version = env!("CARGO_PKG_VERSION"), "slab-server starting");

    let transport = cfg.transport_mode.as_str();
    let serve_http = transport == "http" || transport == "both";
    let serve_grpc = transport == "grpc" || transport == "both";
    if !serve_http && !serve_grpc {
        anyhow::bail!(
            "invalid transport mode '{}'; expected one of: http, grpc, both",
            cfg.transport_mode
        );
    }

    let use_remote_backends = cfg.uses_remote_backends();
    if use_remote_backends {
        info!("remote backend endpoints configured; skipping local slab-core runtime init");
    } else {
        let enabled_backends = parse_enabled_backends(cfg.enabled_backends.as_deref())?;
        let base_lib_path = cfg
            .lib_dir
            .as_deref()
            .unwrap_or(&Path::new("./resources/libs"));
        let llama_lib_dir = enabled_backends.llama.then(|| base_lib_path.join("llama"));
        let whisper_lib_dir = enabled_backends
            .whisper
            .then(|| base_lib_path.join("whisper"));
        let diffusion_lib_dir = enabled_backends
            .diffusion
            .then(|| base_lib_path.join("diffusion"));
        info!(
            enabled_llama = enabled_backends.llama,
            enabled_whisper = enabled_backends.whisper,
            enabled_diffusion = enabled_backends.diffusion,
            llama_lib_dir = ?llama_lib_dir,
            whisper_lib_dir = ?whisper_lib_dir,
            diffusion_lib_dir = ?diffusion_lib_dir,
            "initialising slab-core with backend filters and library paths"
        );

        slab_core::api::init(slab_core::api::Config {
            queue_capacity: cfg.queue_capacity,
            backend_capacity: cfg.backend_capacity,
            llama_lib_dir,
            whisper_lib_dir,
            diffusion_lib_dir,
        })?;
        info!("slab-core runtime initialised");
    }

    if let Err(e) = tokio::fs::create_dir_all(&cfg.session_state_dir).await {
        warn!(
            path = %cfg.session_state_dir,
            error = %e,
            "failed to create session state dir"
        );
    }

    if serve_grpc {
        let grpc_bind = cfg.grpc_bind_address.clone();
        tokio::spawn(async move {
            if let Err(e) = grpc::serve(grpc_bind).await {
                warn!(error = %e, "gRPC server exited");
            }
        });
    }

    if serve_http {
        let store = AnyStore::connect(&cfg.database_url).await?;
        info!(database_url = %cfg.database_url, "database ready");

        let state = Arc::new(AppState {
            config: Arc::new(cfg.clone()),
            store: Arc::new(store.clone()),
            task_manager: Arc::new(TaskManager::new()),
        });

        let app = if use_remote_backends {
            routes::build_gateway(Arc::clone(&state))
        } else {
            routes::build(Arc::clone(&state))
        };
        let addr: SocketAddr = cfg.bind_address.parse()?;
        let listener = tokio::net::TcpListener::bind(addr).await?;
        info!(%addr, "HTTP server listening");
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await?;
        if let Err(e) = store.interrupt_running_tasks().await {
            warn!(
                error = %e,
                "failed to interrupt running tasks on shutdown"
            );
        }
    } else {
        shutdown_signal().await;
    }

    info!("slab-server stopped");
    Ok(())
}

#[derive(Debug)]
struct ManagedChild {
    backend: String,
    bind_address: String,
    child: Child,
}

fn spawn_backend_child(
    exe: &Path,
    backend: &str,
    grpc_bind_address: &str,
    args: &SupervisorArgs,
) -> anyhow::Result<ManagedChild> {
    let session_state_dir = args
        .session_state_base
        .join(format!("slab-sessions-{}", backend));
    std::fs::create_dir_all(&session_state_dir).with_context(|| {
        format!(
            "failed to create session state dir for backend {}: {}",
            backend,
            session_state_dir.display()
        )
    })?;

    let mut cmd = TokioCommand::new(exe);
    cmd.arg("serve")
        .arg("--enabled-backends")
        .arg(backend)
        .arg("--transport")
        .arg("grpc")
        .arg("--grpc-bind")
        .arg(grpc_bind_address)
        .arg("--session-state-dir")
        .arg(session_state_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::null());

    if let Some(v) = &args.database_url {
        cmd.arg("--database-url").arg(v);
    }
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

    let child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn child backend process: {}", backend))?;
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

async fn run_supervisor(args: SupervisorArgs) -> anyhow::Result<()> {
    info!("slab-server supervisor starting");
    let exe = std::env::current_exe().context("failed to resolve current executable path")?;
    let mut children = Vec::new();

    children.push(spawn_backend_child(
        &exe,
        "whisper",
        &args.whisper_bind,
        &args,
    )?);
    children.push(spawn_backend_child(&exe, "llama", &args.llama_bind, &args)?);
    if args.include_diffusion {
        children.push(spawn_backend_child(
            &exe,
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

    let mut gateway_join = tokio::spawn(async move { run_server(gateway_cfg).await });

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
