//! slab-server entry point.
//! Runs in supervisor mode by default.

mod api;
mod error;

use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use clap::Parser;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{error, info, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use slab_app_core::config::{Config, default_model_config_dir_for_settings_path};
use slab_app_core::context::AppState;
use slab_app_core::domain::services::PmidService;
use slab_app_core::infra::db::{AnyStore, TaskStore};
use slab_app_core::infra::rpc::gateway::GrpcGateway;
use slab_app_core::infra::runtime::{ManagedRuntimeHost, ManagedRuntimeHostStartOptions};
use slab_app_core::runtime_supervisor::RuntimeSupervisorStatus;

#[derive(Parser, Debug, Clone, Default)]
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

impl SupervisorArgs {
    fn apply_bootstrap_config(&mut self, cfg: &mut Config) {
        let previous_settings_path = cfg.settings_path.clone();
        let previous_default_model_config_dir =
            default_model_config_dir_for_settings_path(&previous_settings_path);

        if let Some(database_url) = &self.database_url {
            cfg.database_url = database_url.clone();
        }
        if let Some(settings_path) = &self.settings_path {
            cfg.settings_path = settings_path.clone();

            if self.model_config_dir.is_none()
                && cfg.model_config_dir == previous_default_model_config_dir
            {
                cfg.model_config_dir = default_model_config_dir_for_settings_path(settings_path);
            }
        }
        if let Some(model_config_dir) = &self.model_config_dir {
            cfg.model_config_dir = model_config_dir.clone();
        }
        if let Some(log_level) = &self.log_level {
            cfg.log_level = log_level.clone();
        }
        if self.log_json {
            cfg.log_json = true;
        }
        if let Some(log_file) = &self.log_file {
            cfg.log_file = Some(log_file.clone());
        }
        if let Some(lib_dir) = &self.lib_dir {
            cfg.lib_dir = Some(lib_dir.clone());
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
        if self.lib_dir.is_none() {
            self.lib_dir = cfg.lib_dir.clone();
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
        if rejected.is_empty() {
            return Ok(());
        }

        anyhow::bail!(
            "legacy startup override(s) {} are no longer supported. Update settings.json launch.* instead.",
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
    runtime_host: Option<Arc<ManagedRuntimeHost>>,
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
    let grpc = GrpcGateway::connect_from_config_best_effort(&cfg).await;
    info!(?grpc, "shared gRPC gateway services initialized");

    let grpc = Arc::new(grpc);
    let store = Arc::new(store.clone());
    let model_auto_unload =
        Arc::new(slab_app_core::model_auto_unload::ModelAutoUnloadManager::new(
            Arc::clone(&pmid),
            Arc::clone(&grpc),
            Arc::clone(&runtime_status),
        ));
    let state = Arc::new(AppState::new(
        Arc::new(cfg.clone()),
        pmid,
        grpc,
        runtime_status,
        runtime_host,
        Arc::clone(&store),
        model_auto_unload,
    ));
    state.services.model.sync_model_packs_from_disk().await?;

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

async fn run_supervisor(args: SupervisorArgs, mut gateway_cfg: Config) -> anyhow::Result<()> {
    info!("slab-server supervisor starting");
    let runtime_host = Arc::new(
        ManagedRuntimeHost::start_server(
            &gateway_cfg,
            ManagedRuntimeHostStartOptions {
                log_level: args.log_level.clone(),
                log_json: args.log_json,
                ..Default::default()
            },
        )
        .await
        .map_err(anyhow::Error::from)?,
    );
    runtime_host.apply_to_config(&mut gateway_cfg);

    if let Some(error) = runtime_host.startup_error().await {
        warn!(
            error = %error,
            child_count = runtime_host.launch_spec().children.len(),
            gateway_bind = %runtime_host.launch_spec().gateway.as_ref().map(|gateway| gateway.bind_address.as_str()).unwrap_or(""),
            runtime_transport = %runtime_host.launch_spec().transport.as_str(),
            "runtime supervisor unavailable; booting HTTP gateway without managed children"
        );
    } else {
        info!(
            child_count = runtime_host.launch_spec().children.len(),
            gateway_bind = %runtime_host.launch_spec().gateway.as_ref().map(|gateway| gateway.bind_address.as_str()).unwrap_or(""),
            runtime_transport = %runtime_host.launch_spec().transport.as_str(),
            "supervisor started backend children and is booting HTTP gateway"
        );
    }

    let (gateway_shutdown_tx, gateway_shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let mut gateway_shutdown_tx = Some(gateway_shutdown_tx);
    let runtime_status = runtime_host.status_registry();
    let runtime_host_for_gateway = Arc::clone(&runtime_host);
    let mut gateway_join = tokio::spawn(async move {
        run_gateway(gateway_cfg, runtime_status, Some(runtime_host_for_gateway), async move {
            let _ = gateway_shutdown_rx.await;
        })
        .await
    });
    let mut gateway_result_observed = false;
    let shutdown = shutdown_signal(args.shutdown_on_stdin_close);
    tokio::pin!(shutdown);

    let mut result = tokio::select! {
        _ = &mut shutdown => {
            info!("supervisor received shutdown signal");
            Ok(())
        }
        gateway_res = &mut gateway_join => {
            gateway_result_observed = true;
            let result = map_gateway_join_result(gateway_res);
            if let Err(e) = &result {
                error!(
                    error = %e,
                    error_chain = %format_error_chain(e),
                    "HTTP gateway task exited with error"
                );
            }
            result
        }
    };

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

    runtime_host.shutdown().await;
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
    use super::*;
    use slab_app_core::config::{Config, default_model_config_dir_for_settings_path};
    use slab_types::RuntimeBackendId;
    use slab_types::settings::RuntimeTransportMode;
    use std::path::PathBuf;

    #[test]
    fn bootstrap_args_accept_settings_and_database_parameters() {
        let args = SupervisorArgs {
            database_url: Some("sqlite:///tmp/slab.db?mode=rwc".to_owned()),
            settings_path: Some(PathBuf::from("C:/Slab/settings.json")),
            lib_dir: Some(PathBuf::from("C:/Slab/resources/libs")),
            ..SupervisorArgs::default()
        };

        assert!(args.validate_no_legacy_launch_overrides().is_ok());
    }

    #[test]
    fn bootstrap_args_apply_cli_overrides_to_runtime_config() {
        let mut args = SupervisorArgs {
            database_url: Some("sqlite:///tmp/api.db?mode=rwc".to_owned()),
            settings_path: Some(PathBuf::from("D:/Slab/api-settings.json")),
            ..SupervisorArgs::default()
        };
        let mut cfg = Config::from_env();

        cfg.database_url = "sqlite:///tmp/default.db?mode=rwc".to_owned();
        cfg.settings_path = PathBuf::from("C:/Slab/settings.json");
        cfg.model_config_dir = default_model_config_dir_for_settings_path(&cfg.settings_path);

        args.apply_bootstrap_config(&mut cfg);

        assert_eq!(cfg.database_url, "sqlite:///tmp/api.db?mode=rwc");
        assert_eq!(cfg.settings_path, PathBuf::from("D:/Slab/api-settings.json"));
        assert_eq!(cfg.model_config_dir, PathBuf::from("D:/Slab/models"));
        assert_eq!(args.model_config_dir, Some(PathBuf::from("D:/Slab/models")));
    }

    #[test]
    fn bootstrap_args_preserve_explicit_model_config_override() {
        let mut args = SupervisorArgs {
            settings_path: Some(PathBuf::from("D:/Slab/api-settings.json")),
            model_config_dir: Some(PathBuf::from("E:/custom-models")),
            ..SupervisorArgs::default()
        };
        let mut cfg = Config::from_env();

        cfg.settings_path = PathBuf::from("C:/Slab/settings.json");
        cfg.model_config_dir = default_model_config_dir_for_settings_path(&cfg.settings_path);

        args.apply_bootstrap_config(&mut cfg);

        assert_eq!(cfg.settings_path, PathBuf::from("D:/Slab/api-settings.json"));
        assert_eq!(cfg.model_config_dir, PathBuf::from("E:/custom-models"));
    }

    #[test]
    fn bootstrap_args_apply_cli_lib_dir_override_to_runtime_config() {
        let mut args = SupervisorArgs {
            lib_dir: Some(PathBuf::from("C:/Slab/resources/libs")),
            ..SupervisorArgs::default()
        };
        let mut cfg = Config::from_env();

        cfg.lib_dir = Some(PathBuf::from("D:/legacy/libs"));

        args.apply_bootstrap_config(&mut cfg);

        assert_eq!(cfg.lib_dir, Some(PathBuf::from("C:/Slab/resources/libs")));
        assert_eq!(args.lib_dir, Some(PathBuf::from("C:/Slab/resources/libs")));
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

    fn test_child_spec(bind_address: String) -> ResolvedRuntimeChildSpec {
        ResolvedRuntimeChildSpec {
            backend: RuntimeBackendId::GgmlLlama,
            grpc_bind_address: bind_address,
            transport: RuntimeTransportMode::Http,
            queue_capacity: 64,
            backend_capacity: 4,
            lib_dir: None,
            log_level: None,
            log_json: Some(false),
            log_file: PathBuf::from("C:/runtime/logs/slab-runtime-test.log"),
            shutdown_on_stdin_close: true,
        }
    }

    #[tokio::test]
    async fn runtime_endpoint_ready_reports_listening_http_socket() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let spec = test_child_spec(listener.local_addr().unwrap().to_string());

        assert!(runtime_endpoint_ready(&spec).await.unwrap());
    }

    #[tokio::test]
    async fn wait_for_runtime_child_ready_fails_fast_when_child_exits() {
        let spec = test_child_spec("127.0.0.1:9".to_owned());
        let mut cmd = if cfg!(windows) {
            let mut cmd = TokioCommand::new("cmd");
            cmd.args(["/C", "exit 7"]);
            cmd
        } else {
            let mut cmd = TokioCommand::new("sh");
            cmd.args(["-lc", "exit 7"]);
            cmd
        };
        let mut child =
            cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();

        let error = wait_for_runtime_child_ready(&spec, &mut child).await.unwrap_err();
        let message = error.to_string();

        assert!(message.contains("exited before gRPC endpoint"));
    }
}
