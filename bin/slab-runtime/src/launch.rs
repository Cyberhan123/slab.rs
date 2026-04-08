use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll};

use anyhow::Context;
use futures::StreamExt;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, BufReader, ReadBuf};
use tonic::transport::server::Connected;
use tracing::{error, info, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use slab_proto::slab::ipc::v1 as pb;

use crate::config::{Cli, RuntimeConfig};
use crate::context::RuntimeContext;

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

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    let config = Arc::new(cli.into_runtime_config()?);
    let _log_guards = init_tracing(&config.log_level, config.log_json, config.log_file.as_deref())?;
    install_panic_hook();
    log_startup(&config);

    let context = RuntimeContext::new(Arc::clone(&config))?;
    info!(grpc_bind = %config.grpc_bind, "starting slab-runtime gRPC server");
    serve_grpc(
        &config.grpc_bind,
        config.shutdown_on_stdin_close,
        context.grpc_service.clone(),
    )
    .await?;
    info!("slab-runtime stopped");
    Ok(())
}

fn log_startup(config: &RuntimeConfig) {
    info!(
        pid = std::process::id(),
        grpc_bind = %config.grpc_bind,
        enabled_backends = %config.enabled_backends,
        shutdown_on_stdin_close = config.shutdown_on_stdin_close,
        base_lib_path = %config.base_lib_path.display(),
        log_file = ?config.log_file.as_ref().map(|path| path.display().to_string()),
        current_dir = ?std::env::current_dir().ok(),
        current_exe = ?std::env::current_exe().ok(),
        "slab-runtime starting"
    );
    if let Some(path) = &config.llama_lib_dir {
        info!(backend = "llama", lib_dir = %path.display(), exists = path.exists(), "resolved runtime backend library directory");
    }
    if let Some(path) = &config.whisper_lib_dir {
        info!(backend = "whisper", lib_dir = %path.display(), exists = path.exists(), "resolved runtime backend library directory");
    }
    if let Some(path) = &config.diffusion_lib_dir {
        info!(backend = "diffusion", lib_dir = %path.display(), exists = path.exists(), "resolved runtime backend library directory");
    }
    info!(
        queue_capacity = config.queue_capacity,
        backend_capacity = config.backend_capacity,
        "initializing slab-core runtime"
    );
}

fn init_tracing(
    log_level: &str,
    log_json: bool,
    log_file: Option<&Path>,
) -> anyhow::Result<Vec<WorkerGuard>> {
    use tracing_subscriber::Layer;

    let env_filter = match tracing_subscriber::EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(_) => match log_level.parse::<tracing_subscriber::EnvFilter>() {
            Ok(filter) => filter,
            Err(error) => {
                eprintln!("WARN: log level '{log_level}' is invalid ({error}); fallback to info");
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
            Err(error) => {
                if error.kind() != ErrorKind::Interrupted {
                    warn!(
                        error = %error,
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
        if let Err(error) = tokio::signal::ctrl_c().await {
            warn!(error = %error, "failed to install CTRL+C signal handler");
        }
        "ctrl_c"
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => warn!(error = %error, "failed to install SIGTERM handler"),
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
    grpc_service: crate::infra::grpc::GrpcServiceImpl,
) -> anyhow::Result<()> {
    if let Some(raw_ipc_path) = grpc_bind.strip_prefix("ipc://") {
        let ipc_path = raw_ipc_path.trim();
        if ipc_path.is_empty() {
            anyhow::bail!("invalid IPC gRPC endpoint '{}': missing socket/pipe path", grpc_bind);
        }

        #[cfg(unix)]
        {
            if tokio::fs::try_exists(ipc_path).await.unwrap_or(false) {
                if let Err(error) = tokio::fs::remove_file(ipc_path).await {
                    warn!(path = %ipc_path, error = %error, "failed to remove stale IPC socket path before bind");
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
