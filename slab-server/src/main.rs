//! slab-server entry point.
//! Runs in supervisor mode by default.

mod api;
mod config;
mod context;
mod domain;
mod error;
mod infra;
mod model_auto_unload;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use std::{future::Future, io::ErrorKind};

use anyhow::{anyhow, Context};
use clap::Parser;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdin, Command as TokioCommand};
use tracing::{info, warn};

use crate::config::Config;
use crate::context::AppState;
use crate::infra::db::{AnyStore, TaskStore};
use crate::infra::rpc::gateway::GrpcGateway;
use crate::infra::settings::SettingsProvider;

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
    #[arg(long, default_value_t = true)]
    include_diffusion: bool,
    #[arg(long = "runtime-transport")]
    runtime_transport: Option<String>,
    #[arg(long = "runtime-ipc-dir")]
    runtime_ipc_dir: Option<PathBuf>,
    #[arg(long = "database-url")]
    database_url: Option<String>,
    #[arg(long = "settings-path")]
    settings_path: Option<PathBuf>,
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
    #[arg(long, default_value_t = false)]
    shutdown_on_stdin_close: bool,
}

#[derive(Debug, Clone, Copy)]
enum RuntimeTransportMode {
    Http,
    Ipc,
}

impl RuntimeTransportMode {
    fn parse(raw: &str) -> anyhow::Result<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "http" => Ok(Self::Http),
            "both" => Ok(Self::Http),
            "ipc" => Ok(Self::Ipc),
            other => anyhow::bail!(
                "invalid runtime transport '{}'; expected 'http' or 'ipc' ('both' is accepted as an alias of 'http')",
                other
            ),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Ipc => "ipc",
        }
    }
}

#[derive(Debug, Clone)]
struct RuntimeBackendEndpoints {
    whisper: String,
    llama: String,
    diffusion: Option<String>,
}

impl Default for SupervisorArgs {
    fn default() -> Self {
        Self {
            gateway_bind: "127.0.0.1:3000".to_owned(),
            whisper_bind: "127.0.0.1:3001".to_owned(),
            llama_bind: "127.0.0.1:3002".to_owned(),
            diffusion_bind: "127.0.0.1:3003".to_owned(),
            include_diffusion: true,
            runtime_transport: None,
            runtime_ipc_dir: None,
            database_url: None,
            settings_path: None,
            log_level: None,
            log_json: false,
            queue_capacity: None,
            backend_capacity: None,
            lib_dir: None,
            shutdown_on_stdin_close: false,
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
    if args.settings_path.is_none() {
        args.settings_path = Some(cfg.settings_path.clone());
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
    if args.runtime_transport.is_none() {
        args.runtime_transport = Some(cfg.transport_mode.clone());
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

async fn run_gateway<F>(cfg: Config, shutdown: F) -> anyhow::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
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
    let settings = Arc::new(SettingsProvider::load(cfg.settings_path.clone()).await?);
    info!(settings_path = %cfg.settings_path.display(), "settings provider ready");
    let grpc = GrpcGateway::connect_from_config(&cfg)
        .await
        .context("failed to initialize shared gRPC gateway services")?;

    let grpc = Arc::new(grpc);
    let store = Arc::new(store.clone());
    let model_auto_unload = Arc::new(model_auto_unload::ModelAutoUnloadManager::new(
        Arc::clone(&settings),
        Arc::clone(&grpc),
    ));
    let state = Arc::new(AppState::new(
        Arc::new(cfg.clone()),
        grpc,
        Arc::clone(&store),
        settings,
        model_auto_unload,
    ));

    let app = api::build(Arc::clone(&state));
    let addr: SocketAddr = cfg.bind_address.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "HTTP gateway listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
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
    stdin: Option<ChildStdin>,
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
        .arg("--shutdown-on-stdin-close")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::piped());

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

    let mut child = cmd.spawn().with_context(|| {
        format!(
            "failed to spawn slab-runtime child '{}' from {}",
            backend,
            runtime_exe.display()
        )
    })?;
    let stdin = child.stdin.take();
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
        stdin,
    })
}

async fn shutdown_children(children: &mut [ManagedChild]) {
    const GRACEFUL_WAIT: Duration = Duration::from_secs(5);
    const FORCE_WAIT: Duration = Duration::from_secs(5);

    for managed in children.iter_mut() {
        match managed.child.try_wait() {
            Ok(Some(status)) => {
                info!(
                    backend = %managed.backend,
                    bind_address = %managed.bind_address,
                    status = %status,
                    "child process already exited"
                );
                continue;
            }
            Ok(None) => {}
            Err(e) => {
                warn!(
                    backend = %managed.backend,
                    bind_address = %managed.bind_address,
                    error = %e,
                    "failed to query child status before graceful shutdown"
                );
            }
        }

        if managed.stdin.take().is_some() {
            info!(
                backend = %managed.backend,
                bind_address = %managed.bind_address,
                "requested child graceful shutdown via stdin close"
            );
        } else {
            warn!(
                backend = %managed.backend,
                bind_address = %managed.bind_address,
                "child stdin handle missing; will fall back to force kill if needed"
            );
        }

        match tokio::time::timeout(GRACEFUL_WAIT, managed.child.wait()).await {
            Ok(Ok(status)) => {
                info!(
                    backend = %managed.backend,
                    bind_address = %managed.bind_address,
                    status = %status,
                    "child process exited gracefully"
                );
                continue;
            }
            Ok(Err(e)) => {
                warn!(
                    backend = %managed.backend,
                    bind_address = %managed.bind_address,
                    error = %e,
                    "failed while waiting child graceful exit"
                );
            }
            Err(_) => {
                warn!(
                    backend = %managed.backend,
                    bind_address = %managed.bind_address,
                    "timed out waiting for child graceful exit"
                );
            }
        }

        if let Err(e) = managed.child.start_kill() {
            warn!(
                backend = %managed.backend,
                bind_address = %managed.bind_address,
                error = %e,
                "failed to signal child force kill"
            );
            continue;
        }

        match tokio::time::timeout(FORCE_WAIT, managed.child.wait()).await {
            Ok(Ok(status)) => {
                info!(
                    backend = %managed.backend,
                    bind_address = %managed.bind_address,
                    status = %status,
                    "child process exited after force kill"
                );
            }
            Ok(Err(e)) => {
                warn!(
                    backend = %managed.backend,
                    bind_address = %managed.bind_address,
                    error = %e,
                    "failed while waiting child exit after force kill"
                );
            }
            Err(_) => {
                warn!(
                    backend = %managed.backend,
                    bind_address = %managed.bind_address,
                    "timed out waiting for child exit after force kill"
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

fn build_runtime_backend_endpoints(
    args: &SupervisorArgs,
    mode: RuntimeTransportMode,
) -> anyhow::Result<RuntimeBackendEndpoints> {
    match mode {
        RuntimeTransportMode::Http => Ok(RuntimeBackendEndpoints {
            whisper: args.whisper_bind.clone(),
            llama: args.llama_bind.clone(),
            diffusion: args.include_diffusion.then(|| args.diffusion_bind.clone()),
        }),
        RuntimeTransportMode::Ipc => build_ipc_runtime_backend_endpoints(args),
    }
}

#[cfg(windows)]
fn build_ipc_runtime_backend_endpoints(
    args: &SupervisorArgs,
) -> anyhow::Result<RuntimeBackendEndpoints> {
    let pid = std::process::id();
    let whisper = format!(r"ipc://\\.\pipe\slab-runtime-{}-whisper", pid);
    let llama = format!(r"ipc://\\.\pipe\slab-runtime-{}-llama", pid);
    let diffusion = args
        .include_diffusion
        .then(|| format!(r"ipc://\\.\pipe\slab-runtime-{}-diffusion", pid));
    Ok(RuntimeBackendEndpoints {
        whisper,
        llama,
        diffusion,
    })
}

#[cfg(not(windows))]
fn build_ipc_runtime_backend_endpoints(
    args: &SupervisorArgs,
) -> anyhow::Result<RuntimeBackendEndpoints> {
    let base_dir = args
        .runtime_ipc_dir
        .clone()
        .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&base_dir).with_context(|| {
        format!(
            "failed to create runtime IPC socket directory '{}'",
            base_dir.display()
        )
    })?;

    let pid = std::process::id();
    let endpoint_for = |backend: &str| -> String {
        let path = base_dir.join(format!("slab-runtime-{}-{}.sock", pid, backend));
        format!("ipc://{}", path.to_string_lossy())
    };

    Ok(RuntimeBackendEndpoints {
        whisper: endpoint_for("whisper"),
        llama: endpoint_for("llama"),
        diffusion: args.include_diffusion.then(|| endpoint_for("diffusion")),
    })
}

async fn run_supervisor(args: SupervisorArgs) -> anyhow::Result<()> {
    info!("slab-server supervisor starting");
    let server_exe =
        std::env::current_exe().context("failed to resolve current executable path")?;
    let runtime_exe = resolve_runtime_exe(&server_exe)?;
    let runtime_transport =
        RuntimeTransportMode::parse(args.runtime_transport.as_deref().unwrap_or("http"))?;
    let backend_endpoints = build_runtime_backend_endpoints(&args, runtime_transport)?;
    let mut children = Vec::new();

    children.push(spawn_backend_child(
        &runtime_exe,
        "whisper",
        &backend_endpoints.whisper,
        &args,
    )?);
    children.push(spawn_backend_child(
        &runtime_exe,
        "llama",
        &backend_endpoints.llama,
        &args,
    )?);
    if args.include_diffusion {
        let diffusion_endpoint = backend_endpoints.diffusion.as_deref().ok_or_else(|| {
            anyhow!("diffusion endpoint is missing while diffusion backend is enabled")
        })?;
        children.push(spawn_backend_child(
            &runtime_exe,
            "diffusion",
            diffusion_endpoint,
            &args,
        )?);
    }

    info!(
        child_count = children.len(),
        gateway_bind = %args.gateway_bind,
        runtime_transport = %runtime_transport.as_str(),
        "supervisor started backend children and is booting HTTP gateway"
    );

    let mut gateway_cfg = Config::from_env();
    if let Some(v) = &args.database_url {
        gateway_cfg.database_url = v.clone();
    }
    if let Some(v) = &args.settings_path {
        gateway_cfg.settings_path = v.clone();
    }
    if let Some(v) = &args.log_level {
        gateway_cfg.log_level = v.clone();
    }
    if args.log_json {
        gateway_cfg.log_json = true;
    }
    gateway_cfg.bind_address = args.gateway_bind.clone();
    gateway_cfg.transport_mode = runtime_transport.as_str().to_string();
    gateway_cfg.whisper_grpc_endpoint = Some(backend_endpoints.whisper.clone());
    gateway_cfg.llama_grpc_endpoint = Some(backend_endpoints.llama.clone());
    gateway_cfg.diffusion_grpc_endpoint = backend_endpoints.diffusion.clone();

    let (gateway_shutdown_tx, gateway_shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let mut gateway_shutdown_tx = Some(gateway_shutdown_tx);
    let mut gateway_join = tokio::spawn(async move {
        run_gateway(gateway_cfg, async move {
            let _ = gateway_shutdown_rx.await;
        })
        .await
    });
    let mut gateway_result_observed = false;
    let shutdown = shutdown_signal(args.shutdown_on_stdin_close);
    tokio::pin!(shutdown);

    let mut result = Ok(());
    loop {
        tokio::select! {
            _ = &mut shutdown => {
                info!("supervisor received shutdown signal");
                break;
            }
            gateway_res = &mut gateway_join => {
                gateway_result_observed = true;
                result = map_gateway_join_result(gateway_res);
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

    if let Some(tx) = gateway_shutdown_tx.take() {
        let _ = tx.send(());
    }

    if !gateway_result_observed {
        match tokio::time::timeout(Duration::from_secs(5), &mut gateway_join).await {
            Ok(gateway_res) => {
                let gateway_outcome = map_gateway_join_result(gateway_res);
                if result.is_ok() {
                    result = gateway_outcome;
                } else if let Err(e) = gateway_outcome {
                    warn!(error = %e, "gateway shutdown also failed");
                }
            }
            Err(_) => {
                warn!("timed out waiting for gateway graceful shutdown; aborting gateway task");
                gateway_join.abort();
                let _ = gateway_join.await;
            }
        }
    }

    shutdown_children(&mut children).await;
    info!("supervisor stopped");
    result
}

fn map_gateway_join_result(
    gateway_res: Result<anyhow::Result<()>, tokio::task::JoinError>,
) -> anyhow::Result<()> {
    match gateway_res {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(anyhow!("gateway task join error: {e}")),
    }
}

async fn wait_for_stdin_shutdown_signal() {
    let mut reader = BufReader::new(tokio::io::stdin());
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                info!("stdin closed; starting graceful shutdown");
                break;
            }
            Ok(_) => {
                let cmd = line.trim();
                if cmd.eq_ignore_ascii_case("shutdown")
                    || cmd.eq_ignore_ascii_case("exit")
                    || cmd.eq_ignore_ascii_case("quit")
                {
                    info!(command = %cmd, "received shutdown command from stdin");
                    break;
                }
            }
            Err(e) => {
                if e.kind() != ErrorKind::Interrupted {
                    warn!(
                        error = %e,
                        "failed reading stdin for shutdown command; starting shutdown"
                    );
                    break;
                }
            }
        }
    }
}

/// Returns a future that resolves when SIGINT, SIGTERM or optional stdin shutdown signal is received.
async fn shutdown_signal(listen_stdin: bool) {
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

    let stdin_signal = async {
        if listen_stdin {
            wait_for_stdin_shutdown_signal().await;
        } else {
            std::future::pending::<()>().await;
        }
    };

    tokio::select! {
        _ = ctrl_c   => {}
        _ = terminate => {}
        _ = stdin_signal => {}
    }
    info!("shutdown signal received; starting graceful shutdown");
}
