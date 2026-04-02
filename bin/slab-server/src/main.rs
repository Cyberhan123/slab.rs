//! slab-server entry point.
//! Runs in supervisor mode by default.

mod api;
mod error;

use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use anyhow::{Context, anyhow};
use clap::Parser;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdin, Command as TokioCommand};
use tracing::{error, info, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use slab_app_core::config::Config;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::PmidService;
use slab_app_core::infra::db::{AnyStore, TaskStore};
use slab_app_core::infra::rpc::gateway::GrpcGateway;
use slab_app_core::launch::{LaunchHostPaths, LaunchProfile, ResolvedRuntimeChildSpec};
use slab_app_core::runtime_supervisor::{
    ManagedRuntimeSupervisor, RuntimeChildExit, RuntimeChildHandle, RuntimeChildSpawner,
    RuntimeSupervisorOptions, RuntimeSupervisorStatus,
};

#[derive(Parser, Debug, Clone)]
#[command(name = "slab-server", version, about = "Slab supervisor and HTTP gateway")]
struct SupervisorArgs {
    #[arg(long = "database-url")]
    database_url: Option<String>,
    #[arg(long = "settings-path")]
    settings_path: Option<PathBuf>,
    #[arg(long = "model-config-dir")]
    model_config_dir: Option<PathBuf>,
    #[arg(long = "log")]
    log_level: Option<String>,
    #[arg(long = "log-json", action = clap::ArgAction::SetTrue)]
    log_json: bool,
    #[arg(long = "log-file")]
    log_file: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    shutdown_on_stdin_close: bool,
    #[arg(long, hide = true)]
    gateway_bind: Option<String>,
    #[arg(long, hide = true)]
    whisper_bind: Option<String>,
    #[arg(long, hide = true)]
    llama_bind: Option<String>,
    #[arg(long, hide = true)]
    diffusion_bind: Option<String>,
    #[arg(long, hide = true)]
    include_diffusion: Option<bool>,
    #[arg(long = "runtime-transport", hide = true)]
    runtime_transport: Option<String>,
    #[arg(long = "runtime-ipc-dir", hide = true)]
    runtime_ipc_dir: Option<PathBuf>,
    #[arg(long = "queue-capacity", hide = true)]
    queue_capacity: Option<usize>,
    #[arg(long = "backend-capacity", hide = true)]
    backend_capacity: Option<usize>,
    #[arg(long = "lib-dir", hide = true)]
    lib_dir: Option<PathBuf>,
}

impl Default for SupervisorArgs {
    fn default() -> Self {
        Self {
            database_url: None,
            settings_path: None,
            model_config_dir: None,
            log_level: None,
            log_json: false,
            log_file: None,
            shutdown_on_stdin_close: false,
            gateway_bind: None,
            whisper_bind: None,
            llama_bind: None,
            diffusion_bind: None,
            include_diffusion: None,
            runtime_transport: None,
            runtime_ipc_dir: None,
            queue_capacity: None,
            backend_capacity: None,
            lib_dir: None,
        }
    }
}

impl SupervisorArgs {
    fn apply_bootstrap_config(&mut self, cfg: &mut Config) {
        if let Some(log_level) = &self.log_level {
            cfg.log_level = log_level.clone();
        }
        if self.log_json {
            cfg.log_json = true;
        }
        if let Some(log_file) = &self.log_file {
            cfg.log_file = Some(log_file.clone());
        }
        if !self.log_json && cfg.log_json {
            self.log_json = true;
        }
        if self.database_url.is_none() {
            self.database_url = Some(cfg.database_url.clone());
        }
        if self.log_level.is_none() {
            self.log_level = Some(cfg.log_level.clone());
        }
        if self.log_file.is_none() {
            self.log_file = cfg.log_file.clone();
        }
        if self.settings_path.is_none() {
            self.settings_path = Some(cfg.settings_path.clone());
        }
        if self.model_config_dir.is_none() {
            self.model_config_dir = Some(cfg.model_config_dir.clone());
        }
    }

    fn validate_no_legacy_launch_overrides(&self) -> anyhow::Result<()> {
        let mut rejected = Vec::new();

        if self.gateway_bind.is_some() {
            rejected.push("--gateway-bind");
        }
        if self.whisper_bind.is_some() {
            rejected.push("--whisper-bind");
        }
        if self.llama_bind.is_some() {
            rejected.push("--llama-bind");
        }
        if self.diffusion_bind.is_some() {
            rejected.push("--diffusion-bind");
        }
        if self.include_diffusion.is_some() {
            rejected.push("--include-diffusion");
        }
        if self.runtime_transport.is_some() {
            rejected.push("--runtime-transport");
        }
        if self.runtime_ipc_dir.is_some() {
            rejected.push("--runtime-ipc-dir");
        }
        if self.queue_capacity.is_some() {
            rejected.push("--queue-capacity");
        }
        if self.backend_capacity.is_some() {
            rejected.push("--backend-capacity");
        }
        if self.lib_dir.is_some() {
            rejected.push("--lib-dir");
        }

        if rejected.is_empty() {
            return Ok(());
        }

        anyhow::bail!(
            "legacy startup override(s) {} are no longer supported. Update settings.json launch.* (and setup.backends.dir for runtime libraries) instead.",
            rejected.join(", ")
        );
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = SupervisorArgs::parse();
    let mut cfg = Config::from_env();
    args.validate_no_legacy_launch_overrides()?;
    args.apply_bootstrap_config(&mut cfg);

    let _log_guards = init_tracing(&cfg.log_level, cfg.log_json, cfg.log_file.as_deref())?;
    run_supervisor(args, cfg).await
}

fn init_tracing(
    log_level: &str,
    log_json: bool,
    log_file: Option<&Path>,
) -> anyhow::Result<Vec<WorkerGuard>> {
    let env_filter = match tracing_subscriber::EnvFilter::try_from_default_env() {
        Ok(f) => f,
        Err(_) => match log_level.parse::<tracing_subscriber::EnvFilter>() {
            Ok(f) => f,
            Err(e) => {
                eprintln!("WARN: log level '{}' is invalid ({}); fallback to info", log_level, e);
                tracing_subscriber::EnvFilter::new("info")
            }
        },
    };

    let mut guards = Vec::new();

    if let Some(path) = log_file {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                anyhow::anyhow!(
                    "failed to create slab-server log directory '{}': {error}",
                    parent.display()
                )
            })?;
        }

        let file = OpenOptions::new().create(true).append(true).open(path).map_err(|error| {
            anyhow::anyhow!("failed to open slab-server log file '{}': {error}", path.display())
        })?;
        let (file_writer, guard) = tracing_appender::non_blocking(file);
        guards.push(guard);

        if log_json {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    tracing_subscriber::fmt::layer().json().with_target(true).with_thread_ids(true),
                )
                .with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .with_ansi(false)
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_writer(file_writer),
                )
                .init();
        } else {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_subscriber::fmt::layer().with_target(true).with_thread_ids(true))
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_ansi(false)
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_writer(file_writer),
                )
                .init();
        }
    } else if log_json {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json().with_target(true).with_thread_ids(true))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().with_target(true).with_thread_ids(true))
            .init();
    }

    Ok(guards)
}

async fn run_gateway<F>(
    cfg: Config,
    runtime_status: Arc<RuntimeSupervisorStatus>,
    shutdown: F,
) -> anyhow::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    info!(version = env!("CARGO_PKG_VERSION"), "slab-server gateway starting");

    if let Err(e) = tokio::fs::create_dir_all(&cfg.session_state_dir).await {
        warn!(
            path = %cfg.session_state_dir,
            error = %e,
            "failed to create session state dir"
        );
    }

    let store = AnyStore::connect(&cfg.database_url).await?;
    info!(database_url = %cfg.database_url, "database ready");
    let pmid = Arc::new(PmidService::load_from_path(cfg.settings_path.clone()).await?);
    info!(settings_path = %cfg.settings_path.display(), "settings service ready");
    info!(
        model_config_dir = %cfg.model_config_dir.display(),
        "model config directory ready"
    );
    info!("typed PMID config ready");
    let grpc = GrpcGateway::connect_from_config(&cfg)
        .await
        .context("failed to initialize shared gRPC gateway services")?;

    let grpc = Arc::new(grpc);
    let store = Arc::new(store.clone());
    let model_auto_unload =
        Arc::new(slab_app_core::model_auto_unload::ModelAutoUnloadManager::new(
            Arc::clone(&pmid),
            Arc::clone(&grpc),
        ));
    let state = Arc::new(AppState::new(
        Arc::new(cfg.clone()),
        pmid,
        grpc,
        runtime_status,
        Arc::clone(&store),
        model_auto_unload,
    ));
    state.services.model.sync_model_configs_from_disk().await?;

    let app = api::build(Arc::clone(&state));
    let addr: SocketAddr = cfg.bind_address.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "HTTP gateway listening");
    axum::serve(listener, app).with_graceful_shutdown(shutdown).await?;

    if let Err(e) = store.interrupt_running_tasks().await {
        warn!(
            error = %e,
            "failed to interrupt running tasks on shutdown"
        );
    }

    info!("slab-server gateway stopped");
    Ok(())
}

struct TokioRuntimeSpawner {
    runtime_exe: PathBuf,
    log_level: Option<String>,
    log_json: bool,
}

impl TokioRuntimeSpawner {
    fn new(runtime_exe: PathBuf, log_level: Option<String>, log_json: bool) -> Self {
        Self { runtime_exe, log_level, log_json }
    }
}

struct TokioRuntimeChildHandle {
    backend: String,
    bind_address: String,
    child: Child,
    stdin: Option<ChildStdin>,
}

#[async_trait]
impl RuntimeChildHandle for TokioRuntimeChildHandle {
    async fn wait_for_exit(
        &mut self,
    ) -> Result<RuntimeChildExit, slab_app_core::error::AppCoreError> {
        let status = self.child.wait().await.map_err(|error| {
            slab_app_core::error::AppCoreError::Internal(format!(
                "failed to wait for runtime child '{}': {error}",
                self.backend
            ))
        })?;
        Ok(RuntimeChildExit {
            code: status.code(),
            signal: None,
            message: (!status.success()).then(|| format!("process exited with status {status}")),
        })
    }

    async fn request_graceful_shutdown(
        &mut self,
    ) -> Result<(), slab_app_core::error::AppCoreError> {
        if self.stdin.take().is_some() {
            info!(
                backend = %self.backend,
                bind_address = %self.bind_address,
                "requested child graceful shutdown via stdin close"
            );
        } else {
            warn!(
                backend = %self.backend,
                bind_address = %self.bind_address,
                "child stdin handle missing; graceful shutdown may already be in progress"
            );
        }
        Ok(())
    }

    async fn force_kill(&mut self) -> Result<(), slab_app_core::error::AppCoreError> {
        self.child.start_kill().map_err(|error| {
            slab_app_core::error::AppCoreError::Internal(format!(
                "failed to signal child force kill '{}': {error}",
                self.backend
            ))
        })
    }
}

#[async_trait]
impl RuntimeChildSpawner for TokioRuntimeSpawner {
    async fn spawn_child(
        &self,
        child_spec: &ResolvedRuntimeChildSpec,
    ) -> Result<Box<dyn RuntimeChildHandle>, slab_app_core::error::AppCoreError> {
        let mut cmd = TokioCommand::new(&self.runtime_exe);
        cmd.args(child_spec.command_args(self.log_level.as_deref(), self.log_json))
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdin(Stdio::piped());

        let mut child = cmd.spawn().map_err(|error| {
            slab_app_core::error::AppCoreError::Internal(format!(
                "failed to spawn slab-runtime child '{}' from {}: {error}",
                child_spec.backend.canonical_id(),
                self.runtime_exe.display()
            ))
        })?;
        let stdin = child.stdin.take();
        info!(
            backend = child_spec.backend.canonical_id(),
            bind_address = %child_spec.grpc_bind_address,
            pid = ?child.id(),
            log_file = %child_spec.log_file.display(),
            "spawned backend child process"
        );

        Ok(Box::new(TokioRuntimeChildHandle {
            backend: child_spec.backend.canonical_id().to_owned(),
            bind_address: child_spec.grpc_bind_address.clone(),
            child,
            stdin,
        }))
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

async fn run_supervisor(args: SupervisorArgs, mut gateway_cfg: Config) -> anyhow::Result<()> {
    info!("slab-server supervisor starting");
    let server_exe =
        std::env::current_exe().context("failed to resolve current executable path")?;
    let runtime_exe = resolve_runtime_exe(&server_exe)?;

    let runtime_log_dir_fallback = gateway_cfg
        .settings_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::temp_dir().join("Slab"))
        .join("logs");
    let runtime_ipc_dir_fallback = gateway_cfg
        .settings_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::temp_dir().join("Slab"))
        .join("ipc");

    let pmid = PmidService::load_from_path(gateway_cfg.settings_path.clone()).await?;
    let launch_spec = pmid
        .resolve_launch_spec(
            LaunchProfile::Server,
            &LaunchHostPaths {
                runtime_lib_dir_fallback: gateway_cfg.lib_dir.clone(),
                runtime_log_dir_fallback,
                runtime_ipc_dir_fallback,
                shutdown_on_stdin_close: true,
            },
        )
        .await?;
    launch_spec.prepare_filesystem().map_err(anyhow::Error::from)?;

    let runtime_supervisor = Arc::new(
        ManagedRuntimeSupervisor::start(
            launch_spec,
            Arc::new(TokioRuntimeSpawner::new(
                runtime_exe,
                args.log_level.clone(),
                args.log_json,
            )),
            RuntimeSupervisorOptions::default(),
        )
        .await
        .map_err(anyhow::Error::from)?,
    );

    info!(
        child_count = runtime_supervisor.launch_spec().children.len(),
        gateway_bind = %runtime_supervisor.launch_spec().gateway.as_ref().map(|gateway| gateway.bind_address.as_str()).unwrap_or(""),
        runtime_transport = %runtime_supervisor.launch_spec().transport.as_str(),
        "supervisor started backend children and is booting HTTP gateway"
    );
    runtime_supervisor.launch_spec().apply_to_config(&mut gateway_cfg);

    let (gateway_shutdown_tx, gateway_shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let mut gateway_shutdown_tx = Some(gateway_shutdown_tx);
    let runtime_status = runtime_supervisor.status_registry();
    let mut gateway_join = tokio::spawn(async move {
        run_gateway(gateway_cfg, runtime_status, async move {
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
                if let Err(e) = &result {
                    error!(
                        error = %e,
                        error_chain = %format_error_chain(e),
                        "HTTP gateway task exited with error"
                    );
                }
                break;
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

    runtime_supervisor.shutdown().await;
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

fn format_error_chain(error: &anyhow::Error) -> String {
    error
        .chain()
        .enumerate()
        .map(|(index, cause)| format!("[{index}] {cause}"))
        .collect::<Vec<_>>()
        .join(" -> ")
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
        use tokio::signal::unix::{SignalKind, signal};
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

#[cfg(test)]
mod tests {
    use super::SupervisorArgs;
    use std::path::PathBuf;

    #[test]
    fn bootstrap_args_accept_settings_and_database_parameters() {
        let args = SupervisorArgs {
            database_url: Some("sqlite:///tmp/slab.db?mode=rwc".to_owned()),
            settings_path: Some(PathBuf::from("C:/Slab/settings.json")),
            ..SupervisorArgs::default()
        };

        assert!(args.validate_no_legacy_launch_overrides().is_ok());
    }

    #[test]
    fn legacy_launch_overrides_are_rejected() {
        let args = SupervisorArgs {
            gateway_bind: Some("127.0.0.1:9000".to_owned()),
            queue_capacity: Some(32),
            ..SupervisorArgs::default()
        };

        let error = args.validate_no_legacy_launch_overrides().expect_err("legacy args must fail");
        let message = error.to_string();
        assert!(message.contains("--gateway-bind"));
        assert!(message.contains("--queue-capacity"));
    }
}
