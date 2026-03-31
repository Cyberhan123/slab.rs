mod grpc;

use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};

use anyhow::Context;
use clap::Parser;
use futures::StreamExt;
use slab_proto::slab::ipc::v1 as pb;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, BufReader, ReadBuf};
use tonic::transport::server::Connected;
use tracing::{error, info, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Parser, Debug, Clone)]
#[command(name = "slab-runtime", version, about = "Slab gRPC runtime worker")]
struct Cli {
    #[arg(long = "grpc-bind", default_value = "127.0.0.1:50051")]
    grpc_bind: String,
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
    #[arg(long = "log-file")]
    log_file: Option<PathBuf>,
    #[arg(long = "enabled-backends")]
    enabled_backends: Option<String>,
    #[arg(long, default_value_t = false)]
    shutdown_on_stdin_close: bool,
}

#[derive(Debug, Clone, Copy)]
struct EnabledBackends {
    llama: bool,
    whisper: bool,
    diffusion: bool,
}

#[derive(Debug)]
struct IpcIo<T> {
    inner: T,
}

impl<T> IpcIo<T> {
    fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T> Connected for IpcIo<T> {
    type ConnectInfo = ();

    fn connect_info(&self) -> Self::ConnectInfo {}
}

impl<T: AsyncRead + Unpin> AsyncRead for IpcIo<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for IpcIo<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl EnabledBackends {
    fn all() -> Self {
        Self { llama: true, whisper: true, diffusion: true }
    }
}

impl std::fmt::Display for EnabledBackends {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for name in [
            self.llama.then_some("llama"),
            self.whisper.then_some("whisper"),
            self.diffusion.then_some("diffusion"),
        ]
        .into_iter()
        .flatten()
        {
            if !first {
                f.write_str(",")?;
            }
            f.write_str(name)?;
            first = false;
        }
        Ok(())
    }
}

fn parse_enabled_backends(raw: Option<&str>) -> anyhow::Result<EnabledBackends> {
    let Some(raw) = raw.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(EnabledBackends::all());
    };

    let mut enabled = EnabledBackends { llama: false, whisper: false, diffusion: false };
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
            "invalid enabled backends: {}. Supported: llama, whisper, diffusion",
            unknown.join(", ")
        );
    }
    if !enabled.llama && !enabled.whisper && !enabled.diffusion {
        anyhow::bail!("at least one backend must be enabled");
    }

    Ok(enabled)
}

fn init_tracing(
    log_level: &str,
    log_json: bool,
    log_file: Option<&Path>,
) -> anyhow::Result<Vec<WorkerGuard>> {
    use tracing_subscriber::Layer;

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

    let stdout_layer: Box<dyn Layer<_> + Send + Sync> = if log_json {
        Box::new(tracing_subscriber::fmt::layer().json().with_target(true).with_thread_ids(true))
    } else {
        Box::new(tracing_subscriber::fmt::layer().with_target(true).with_thread_ids(true))
    };

    let mut guards = Vec::new();

    let registry = tracing_subscriber::registry().with(env_filter).with(stdout_layer);

    if let Some(path) = log_file {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create slab-runtime log directory '{}'", parent.display())
            })?;
        }

        let file = OpenOptions::new().create(true).append(true).open(path).with_context(|| {
            format!("failed to open slab-runtime log file '{}'", path.display())
        })?;
        let (file_writer, guard) = tracing_appender::non_blocking(file);
        guards.push(guard);

        let file_layer: Box<dyn Layer<_> + Send + Sync> = if log_json {
            Box::new(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_ansi(false)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_writer(file_writer),
            )
        } else {
            Box::new(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_writer(file_writer),
            )
        };

        registry.with(file_layer).init();
    } else {
        registry.init();
    }

    Ok(guards)
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        let location = panic_info
            .location()
            .map(|location| {
                format!("{}:{}:{}", location.file(), location.line(), location.column())
            })
            .unwrap_or_else(|| "<unknown>".to_string());
        let payload = if let Some(msg) = panic_info.payload().downcast_ref::<&str>() {
            (*msg).to_string()
        } else if let Some(msg) = panic_info.payload().downcast_ref::<String>() {
            msg.clone()
        } else {
            "non-string panic payload".to_string()
        };

        eprintln!("slab-runtime panic at {location}: {payload}");
        error!(location = %location, payload = %payload, "slab-runtime panicked");
    }));
}

async fn wait_for_stdin_shutdown_signal() {
    let mut reader = BufReader::new(tokio::io::stdin());
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                info!("stdin closed; shutting down runtime");
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
                        "failed reading stdin for shutdown command; shutting down runtime"
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
        "ctrl_c"
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
        "sigterm"
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<&'static str>();

    let stdin_signal = async {
        if listen_stdin {
            wait_for_stdin_shutdown_signal().await;
            "stdin"
        } else {
            std::future::pending::<&'static str>().await
        }
    };

    let source = tokio::select! {
        source = ctrl_c => source,
        source = terminate => source,
        source = stdin_signal => source,
    };
    info!(source, "shutdown signal received; shutting down runtime");
}

async fn serve_grpc(
    grpc_bind: &str,
    shutdown_on_stdin_close: bool,
    grpc_service: grpc::GrpcServiceImpl,
) -> anyhow::Result<()> {
    if let Some(raw_ipc_path) = grpc_bind.strip_prefix("ipc://") {
        let ipc_path = raw_ipc_path.trim();
        if ipc_path.is_empty() {
            anyhow::bail!("invalid IPC gRPC endpoint '{}': missing socket/pipe path", grpc_bind);
        }

        #[cfg(unix)]
        {
            if tokio::fs::try_exists(ipc_path).await.unwrap_or(false) {
                if let Err(e) = tokio::fs::remove_file(ipc_path).await {
                    warn!(path = %ipc_path, error = %e, "failed to remove stale IPC socket path before bind");
                }
            }
        }

        info!(transport = "ipc", path = %ipc_path, "slab-runtime gRPC listening");
        let incoming = parity_tokio_ipc::Endpoint::new(ipc_path.to_owned())
            .incoming()
            .with_context(|| format!("failed to bind IPC endpoint '{ipc_path}'"))?
            .map(|stream| stream.map(IpcIo::new));

        tonic::transport::Server::builder()
            .add_service(pb::llama_service_server::LlamaServiceServer::new(grpc_service.clone()))
            .add_service(pb::whisper_service_server::WhisperServiceServer::new(
                grpc_service.clone(),
            ))
            .add_service(pb::diffusion_service_server::DiffusionServiceServer::new(
                grpc_service.clone(),
            ))
            .serve_with_incoming_shutdown(incoming, shutdown_signal(shutdown_on_stdin_close))
            .await?;
        info!(transport = "ipc", path = %ipc_path, "slab-runtime gRPC server stopped");
        return Ok(());
    }

    let addr = grpc_bind
        .parse()
        .with_context(|| format!("invalid TCP gRPC bind address '{grpc_bind}'"))?;
    info!(transport = "http", %addr, "slab-runtime gRPC listening");
    tonic::transport::Server::builder()
        .add_service(pb::llama_service_server::LlamaServiceServer::new(grpc_service.clone()))
        .add_service(pb::whisper_service_server::WhisperServiceServer::new(grpc_service.clone()))
        .add_service(pb::diffusion_service_server::DiffusionServiceServer::new(grpc_service))
        .serve_with_shutdown(addr, shutdown_signal(shutdown_on_stdin_close))
        .await?;
    info!(transport = "http", %addr, "slab-runtime gRPC server stopped");
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let log_level = cli.log_level.clone().unwrap_or_else(|| "info".to_owned());
    let _log_guards = init_tracing(&log_level, cli.log_json, cli.log_file.as_deref())?;
    install_panic_hook();

    let enabled = parse_enabled_backends(cli.enabled_backends.as_deref())?;
    let base_lib_path = cli.lib_dir.as_deref().unwrap_or(Path::new("./resources/libs"));
    let llama_lib_dir = enabled.llama.then(|| base_lib_path.join("llama"));
    let whisper_lib_dir = enabled.whisper.then(|| base_lib_path.join("whisper"));
    let diffusion_lib_dir = enabled.diffusion.then(|| base_lib_path.join("diffusion"));

    info!(
        pid = std::process::id(),
        grpc_bind = %cli.grpc_bind,
        enabled_backends = %enabled,
        shutdown_on_stdin_close = cli.shutdown_on_stdin_close,
        base_lib_path = %base_lib_path.display(),
        log_file = ?cli.log_file.as_ref().map(|path| path.display().to_string()),
        current_dir = ?std::env::current_dir().ok(),
        current_exe = ?std::env::current_exe().ok(),
        "slab-runtime starting"
    );
    if let Some(path) = &llama_lib_dir {
        info!(backend = "llama", lib_dir = %path.display(), exists = path.exists(), "resolved runtime backend library directory");
    }
    if let Some(path) = &whisper_lib_dir {
        info!(backend = "whisper", lib_dir = %path.display(), exists = path.exists(), "resolved runtime backend library directory");
    }
    if let Some(path) = &diffusion_lib_dir {
        info!(backend = "diffusion", lib_dir = %path.display(), exists = path.exists(), "resolved runtime backend library directory");
    }
    info!(
        queue_capacity = cli.queue_capacity.unwrap_or(64),
        backend_capacity = cli.backend_capacity.unwrap_or(4),
        "initializing slab-core runtime"
    );

    let drivers = slab_core::api::DriversConfig {
        llama_lib_dir,
        whisper_lib_dir,
        diffusion_lib_dir,
        enable_candle_llama: false,
        enable_candle_whisper: false,
        enable_candle_diffusion: false,
        onnx_enabled: false,
    };
    let runtime = slab_core::api::RuntimeBuilder::new()
        .queue_capacity(cli.queue_capacity.unwrap_or(64))
        .backend_capacity(cli.backend_capacity.unwrap_or(4))
        .drivers(drivers.clone())
        .build()
        .context("failed to initialize slab-core runtime")?;
    info!("slab-core runtime initialized");

    let grpc_service = grpc::GrpcServiceImpl::new(runtime, drivers, enabled);
    info!(grpc_bind = %cli.grpc_bind, "starting slab-runtime gRPC server");
    serve_grpc(&cli.grpc_bind, cli.shutdown_on_stdin_close, grpc_service).await?;
    info!("slab-runtime stopped");
    Ok(())
}
