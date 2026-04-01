use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{future::Future, io::ErrorKind};

use anyhow::{Context, anyhow};
use clap::Parser;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdin, Command as TokioCommand};
use tracing::{error, info, warn};

use crate::config::Config;
use crate::context::AppState;
use crate::infra::db::{AnyStore, TaskStore};
use crate::infra::rpc::gateway::GrpcGateway;
use crate::infra::settings::SettingsProvider;

const CHILD_POLL_INTERVAL: Duration = Duration::from_millis(500);
const CHILD_RESTART_DELAY: Duration = Duration::from_secs(1);
const CHILD_SHUTDOWN_GRACE: Duration = Duration::from_secs(5);
const CHILD_FORCE_KILL_WAIT: Duration = Duration::from_secs(5);
const GATEWAY_SHUTDOWN_WAIT: Duration = Duration::from_secs(5);

#[derive(Parser, Debug, Clone)]
#[command(name = "slab-server", version, about = "Slab supervisor and HTTP gateway")]
pub struct SupervisorArgs {
    #[arg(long, default_value = "127.0.0.1:3000")]
    pub gateway_bind: String,
    #[arg(long, default_value = "127.0.0.1:3001")]
    pub whisper_bind: String,
    #[arg(long, default_value = "127.0.0.1:3002")]
    pub llama_bind: String,
    #[arg(long, default_value = "127.0.0.1:3003")]
    pub diffusion_bind: String,
    #[arg(long, default_value_t = true)]
    pub include_diffusion: bool,
    #[arg(long = "runtime-transport")]
    pub runtime_transport: Option<String>,
    #[arg(long = "runtime-ipc-dir")]
    pub runtime_ipc_dir: Option<PathBuf>,
    #[arg(long = "database-url")]
    pub database_url: Option<String>,
    #[arg(long = "settings-path")]
    pub settings_path: Option<PathBuf>,
    #[arg(long = "model-config-dir")]
    pub model_config_dir: Option<PathBuf>,
    #[arg(long = "log")]
    pub log_level: Option<String>,
    #[arg(long = "log-json", action = clap::ArgAction::SetTrue)]
    pub log_json: bool,
    #[arg(long = "log-file")]
    pub log_file: Option<PathBuf>,
    #[arg(long = "queue-capacity")]
    pub queue_capacity: Option<usize>,
    #[arg(long = "backend-capacity")]
    pub backend_capacity: Option<usize>,
    #[arg(long = "lib-dir")]
    pub lib_dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub shutdown_on_stdin_close: bool,
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
            model_config_dir: None,
            log_level: None,
            log_json: false,
            log_file: None,
            queue_capacity: None,
            backend_capacity: None,
            lib_dir: None,
            shutdown_on_stdin_close: false,
        }
    }
}

impl SupervisorArgs {
    pub fn apply_config_defaults(&mut self, cfg: &mut Config) {
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
        if self.queue_capacity.is_none() {
            self.queue_capacity = Some(cfg.queue_capacity);
        }
        if self.backend_capacity.is_none() {
            self.backend_capacity = Some(cfg.backend_capacity);
        }
        if self.lib_dir.is_none() {
            self.lib_dir = cfg.lib_dir.clone();
        }
        if self.runtime_transport.is_none() {
            self.runtime_transport = Some(cfg.transport_mode.clone());
        }
    }
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

#[derive(Debug, Clone)]
struct ChildSpec {
    backend: String,
    bind_address: String,
}

impl ChildSpec {
    fn new(backend: impl Into<String>, bind_address: impl Into<String>) -> Self {
        Self { backend: backend.into(), bind_address: bind_address.into() }
    }
}

#[derive(Debug)]
struct ManagedChild {
    child: Child,
    stdin: Option<ChildStdin>,
}

#[derive(Debug)]
struct ChildSlot {
    spec: ChildSpec,
    process: Option<ManagedChild>,
    restart_attempts: u32,
    next_restart_at: Instant,
}

impl ChildSlot {
    fn spawn(spec: ChildSpec, runtime_exe: &Path, args: &SupervisorArgs) -> anyhow::Result<Self> {
        let process = spawn_backend_child(runtime_exe, &spec.backend, &spec.bind_address, args)?;
        Ok(Self {
            spec,
            process: Some(process),
            restart_attempts: 0,
            next_restart_at: Instant::now(),
        })
    }

    fn observe_exit(&mut self) {
        let Some(process) = self.process.as_mut() else {
            return;
        };

        match process.child.try_wait() {
            Ok(Some(status)) => {
                error!(
                    backend = %self.spec.backend,
                    bind_address = %self.spec.bind_address,
                    status = %status,
                    restart_delay_ms = CHILD_RESTART_DELAY.as_millis(),
                    "backend child exited unexpectedly; supervisor will restart it"
                );
                self.process = None;
                self.next_restart_at = Instant::now() + CHILD_RESTART_DELAY;
            }
            Ok(None) => {}
            Err(e) => {
                warn!(
                    backend = %self.spec.backend,
                    bind_address = %self.spec.bind_address,
                    error = %e,
                    "failed to query backend child status; keeping current process handle"
                );
            }
        }
    }

    fn restart_if_due(&mut self, runtime_exe: &Path, args: &SupervisorArgs) {
        if self.process.is_some() || Instant::now() < self.next_restart_at {
            return;
        }

        self.restart_attempts = self.restart_attempts.saturating_add(1);
        match spawn_backend_child(runtime_exe, &self.spec.backend, &self.spec.bind_address, args) {
            Ok(process) => {
                info!(
                    backend = %self.spec.backend,
                    bind_address = %self.spec.bind_address,
                    restart_attempt = self.restart_attempts,
                    "backend child restarted successfully"
                );
                self.process = Some(process);
                self.restart_attempts = 0;
                self.next_restart_at = Instant::now();
            }
            Err(e) => {
                self.next_restart_at = Instant::now() + CHILD_RESTART_DELAY;
                error!(
                    backend = %self.spec.backend,
                    bind_address = %self.spec.bind_address,
                    restart_attempt = self.restart_attempts,
                    error = %e,
                    error_chain = %format_error_chain(&e),
                    retry_in_ms = CHILD_RESTART_DELAY.as_millis(),
                    "failed to restart backend child; will retry while keeping gateway online"
                );
            }
        }
    }

    async fn shutdown(&mut self) {
        let Some(mut process) = self.process.take() else {
            return;
        };

        match process.child.try_wait() {
            Ok(Some(status)) => {
                info!(
                    backend = %self.spec.backend,
                    bind_address = %self.spec.bind_address,
                    status = %status,
                    "child process already exited"
                );
                return;
            }
            Ok(None) => {}
            Err(e) => {
                warn!(
                    backend = %self.spec.backend,
                    bind_address = %self.spec.bind_address,
                    error = %e,
                    "failed to query child status before graceful shutdown"
                );
            }
        }

        if process.stdin.take().is_some() {
            info!(
                backend = %self.spec.backend,
                bind_address = %self.spec.bind_address,
                "requested child graceful shutdown via stdin close"
            );
        } else {
            warn!(
                backend = %self.spec.backend,
                bind_address = %self.spec.bind_address,
                "child stdin handle missing; will fall back to force kill if needed"
            );
        }

        match tokio::time::timeout(CHILD_SHUTDOWN_GRACE, process.child.wait()).await {
            Ok(Ok(status)) => {
                info!(
                    backend = %self.spec.backend,
                    bind_address = %self.spec.bind_address,
                    status = %status,
                    "child process exited gracefully"
                );
                return;
            }
            Ok(Err(e)) => {
                warn!(
                    backend = %self.spec.backend,
                    bind_address = %self.spec.bind_address,
                    error = %e,
                    "failed while waiting child graceful exit"
                );
            }
            Err(_) => {
                warn!(
                    backend = %self.spec.backend,
                    bind_address = %self.spec.bind_address,
                    "timed out waiting for child graceful exit"
                );
            }
        }

        if let Err(e) = process.child.start_kill() {
            warn!(
                backend = %self.spec.backend,
                bind_address = %self.spec.bind_address,
                error = %e,
                "failed to signal child force kill"
            );
            return;
        }

        match tokio::time::timeout(CHILD_FORCE_KILL_WAIT, process.child.wait()).await {
            Ok(Ok(status)) => {
                info!(
                    backend = %self.spec.backend,
                    bind_address = %self.spec.bind_address,
                    status = %status,
                    "child process exited after force kill"
                );
            }
            Ok(Err(e)) => {
                warn!(
                    backend = %self.spec.backend,
                    bind_address = %self.spec.bind_address,
                    error = %e,
                    "failed while waiting child exit after force kill"
                );
            }
            Err(_) => {
                warn!(
                    backend = %self.spec.backend,
                    bind_address = %self.spec.bind_address,
                    "timed out waiting for child exit after force kill"
                );
            }
        }
    }
}

pub async fn run(args: SupervisorArgs) -> anyhow::Result<()> {
    info!("slab-server supervisor starting");
    let server_exe =
        std::env::current_exe().context("failed to resolve current executable path")?;
    let runtime_exe = resolve_runtime_exe(&server_exe)?;
    let runtime_transport =
        RuntimeTransportMode::parse(args.runtime_transport.as_deref().unwrap_or("http"))?;
    let backend_endpoints = build_runtime_backend_endpoints(&args, runtime_transport)?;
    let child_specs = build_child_specs(&args, &backend_endpoints)?;
    let mut children = child_specs
        .into_iter()
        .map(|spec| ChildSlot::spawn(spec, &runtime_exe, &args))
        .collect::<anyhow::Result<Vec<_>>>()?;

    info!(
        child_count = children.len(),
        gateway_bind = %args.gateway_bind,
        runtime_transport = %runtime_transport.as_str(),
        "supervisor started backend children and is booting HTTP gateway"
    );

    let gateway_cfg = build_gateway_config(&args, runtime_transport, &backend_endpoints);
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
                if let Err(e) = &result {
                    error!(
                        error = %e,
                        error_chain = %format_error_chain(e),
                        "HTTP gateway task exited with error"
                    );
                }
                break;
            }
            _ = tokio::time::sleep(CHILD_POLL_INTERVAL) => {
                for child in &mut children {
                    child.observe_exit();
                }
                for child in &mut children {
                    child.restart_if_due(&runtime_exe, &args);
                }
            }
        }
    }

    if let Some(tx) = gateway_shutdown_tx.take() {
        let _ = tx.send(());
    }

    if !gateway_result_observed {
        match tokio::time::timeout(GATEWAY_SHUTDOWN_WAIT, &mut gateway_join).await {
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

async fn run_gateway<F>(cfg: Config, shutdown: F) -> anyhow::Result<()>
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
    let settings = Arc::new(SettingsProvider::load(cfg.settings_path.clone()).await?);
    info!(settings_path = %cfg.settings_path.display(), "settings provider ready");
    info!(
        model_config_dir = %cfg.model_config_dir.display(),
        "model config directory ready"
    );
    let pmid = Arc::new(crate::domain::services::PmidService::load(Arc::clone(&settings)).await?);
    info!("typed PMID config ready");
    let grpc = GrpcGateway::connect_from_config(&cfg)
        .await
        .context("failed to initialize shared gRPC gateway services")?;

    let grpc = Arc::new(grpc);
    let store = Arc::new(store.clone());
    let model_auto_unload = Arc::new(crate::model_auto_unload::ModelAutoUnloadManager::new(
        Arc::clone(&pmid),
        Arc::clone(&grpc),
    ));
    let state = Arc::new(AppState::new(
        Arc::new(cfg.clone()),
        pmid,
        grpc,
        Arc::clone(&store),
        model_auto_unload,
    ));
    state.services.model.sync_model_configs_from_disk().await?;

    let app = crate::api::build(Arc::clone(&state));
    let addr: SocketAddr = cfg.bind_address.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "HTTP gateway listening");
    axum::serve(listener, app).with_graceful_shutdown(shutdown).await?;

    if let Err(e) = store.interrupt_running_tasks().await {
        warn!(error = %e, "failed to interrupt running tasks on shutdown");
    }

    info!("slab-server gateway stopped");
    Ok(())
}

fn spawn_backend_child(
    runtime_exe: &Path,
    backend: &str,
    grpc_bind_address: &str,
    args: &SupervisorArgs,
) -> anyhow::Result<ManagedChild> {
    let runtime_log_path = runtime_log_file_path(args, backend);
    let mut cmd = TokioCommand::new(runtime_exe);
    cmd.arg("--enabled-backends")
        .arg(backend)
        .arg("--grpc-bind")
        .arg(grpc_bind_address)
        .arg("--shutdown-on-stdin-close")
        .arg("--log-file")
        .arg(&runtime_log_path)
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
        format!("failed to spawn slab-runtime child '{}' from {}", backend, runtime_exe.display())
    })?;
    let stdin = child.stdin.take();
    info!(
        backend = backend,
        bind_address = grpc_bind_address,
        pid = ?child.id(),
        log_file = %runtime_log_path.display(),
        "spawned backend child process"
    );
    Ok(ManagedChild { child, stdin })
}

fn runtime_log_file_path(args: &SupervisorArgs, backend: &str) -> PathBuf {
    let base_dir = args
        .settings_path
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::temp_dir().join("Slab"));
    let logs_dir = base_dir.join("logs");
    if let Err(e) = std::fs::create_dir_all(&logs_dir) {
        warn!(
            path = %logs_dir.display(),
            error = %e,
            "failed to create runtime log directory; falling back to temp dir"
        );
        let fallback_dir = std::env::temp_dir().join("Slab").join("logs");
        if let Err(fallback_error) = std::fs::create_dir_all(&fallback_dir) {
            warn!(
                path = %fallback_dir.display(),
                error = %fallback_error,
                "failed to create fallback runtime log directory"
            );
            return std::env::temp_dir().join(format!(
                "slab-runtime-{}-{}.log",
                std::process::id(),
                backend
            ));
        }
        return fallback_dir.join(format!("slab-runtime-{}-{}.log", std::process::id(), backend));
    }

    logs_dir.join(format!("slab-runtime-{}-{}.log", std::process::id(), backend))
}

async fn shutdown_children(children: &mut [ChildSlot]) {
    for child in children {
        child.shutdown().await;
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

fn build_gateway_config(
    args: &SupervisorArgs,
    runtime_transport: RuntimeTransportMode,
    backend_endpoints: &RuntimeBackendEndpoints,
) -> Config {
    let mut gateway_cfg = Config::from_env();
    if let Some(v) = &args.database_url {
        gateway_cfg.database_url = v.clone();
    }
    if let Some(v) = &args.settings_path {
        gateway_cfg.settings_path = v.clone();
    }
    if let Some(v) = &args.model_config_dir {
        gateway_cfg.model_config_dir = v.clone();
    }
    if let Some(v) = &args.log_level {
        gateway_cfg.log_level = v.clone();
    }
    if args.log_json {
        gateway_cfg.log_json = true;
    }
    if let Some(v) = &args.log_file {
        gateway_cfg.log_file = Some(v.clone());
    }
    gateway_cfg.bind_address = args.gateway_bind.clone();
    gateway_cfg.transport_mode = runtime_transport.as_str().to_string();
    gateway_cfg.whisper_grpc_endpoint = Some(backend_endpoints.whisper.clone());
    gateway_cfg.llama_grpc_endpoint = Some(backend_endpoints.llama.clone());
    gateway_cfg.diffusion_grpc_endpoint = backend_endpoints.diffusion.clone();
    gateway_cfg
}

fn build_child_specs(
    args: &SupervisorArgs,
    backend_endpoints: &RuntimeBackendEndpoints,
) -> anyhow::Result<Vec<ChildSpec>> {
    let mut specs = vec![
        ChildSpec::new("ggml.whisper", backend_endpoints.whisper.clone()),
        ChildSpec::new("ggml.llama", backend_endpoints.llama.clone()),
    ];

    if args.include_diffusion {
        let diffusion_endpoint = backend_endpoints.diffusion.as_deref().ok_or_else(|| {
            anyhow!("diffusion endpoint is missing while diffusion backend is enabled")
        })?;
        specs.push(ChildSpec::new("ggml.diffusion", diffusion_endpoint));
    }

    Ok(specs)
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
    let diffusion =
        args.include_diffusion.then(|| format!(r"ipc://\\.\pipe\slab-runtime-{}-diffusion", pid));
    Ok(RuntimeBackendEndpoints { whisper, llama, diffusion })
}

#[cfg(not(windows))]
fn build_ipc_runtime_backend_endpoints(
    args: &SupervisorArgs,
) -> anyhow::Result<RuntimeBackendEndpoints> {
    let base_dir = args.runtime_ipc_dir.clone().unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&base_dir).with_context(|| {
        format!("failed to create runtime IPC socket directory '{}'", base_dir.display())
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
        _ = ctrl_c => {}
        _ = terminate => {}
        _ = stdin_signal => {}
    }
    info!("shutdown signal received; starting graceful shutdown");
}
