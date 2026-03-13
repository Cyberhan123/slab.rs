mod grpc;

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
use tracing::{info, warn};

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
            "invalid enabled backends: {}. Supported: llama, whisper, diffusion",
            unknown.join(", ")
        );
    }
    if !enabled.llama && !enabled.whisper && !enabled.diffusion {
        anyhow::bail!("at least one backend must be enabled");
    }

    Ok(enabled)
}

fn init_tracing(log_level: &str, log_json: bool) {
    let env_filter = match tracing_subscriber::EnvFilter::try_from_default_env() {
        Ok(f) => f,
        Err(_) => match log_level.parse::<tracing_subscriber::EnvFilter>() {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "WARN: log level '{}' is invalid ({}); fallback to info",
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
        _ = ctrl_c => {}
        _ = terminate => {}
        _ = stdin_signal => {}
    }
}

async fn serve_grpc(grpc_bind: &str, shutdown_on_stdin_close: bool) -> anyhow::Result<()> {
    if let Some(raw_ipc_path) = grpc_bind.strip_prefix("ipc://") {
        let ipc_path = raw_ipc_path.trim();
        if ipc_path.is_empty() {
            anyhow::bail!(
                "invalid IPC gRPC endpoint '{}': missing socket/pipe path",
                grpc_bind
            );
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
            .add_service(pb::llama_service_server::LlamaServiceServer::new(
                grpc::GrpcServiceImpl,
            ))
            .add_service(pb::whisper_service_server::WhisperServiceServer::new(
                grpc::GrpcServiceImpl,
            ))
            .add_service(pb::diffusion_service_server::DiffusionServiceServer::new(
                grpc::GrpcServiceImpl,
            ))
            .serve_with_incoming_shutdown(incoming, shutdown_signal(shutdown_on_stdin_close))
            .await?;
        return Ok(());
    }

    let addr = grpc_bind
        .parse()
        .with_context(|| format!("invalid TCP gRPC bind address '{grpc_bind}'"))?;
    info!(transport = "http", %addr, "slab-runtime gRPC listening");
    tonic::transport::Server::builder()
        .add_service(pb::llama_service_server::LlamaServiceServer::new(
            grpc::GrpcServiceImpl,
        ))
        .add_service(pb::whisper_service_server::WhisperServiceServer::new(
            grpc::GrpcServiceImpl,
        ))
        .add_service(pb::diffusion_service_server::DiffusionServiceServer::new(
            grpc::GrpcServiceImpl,
        ))
        .serve_with_shutdown(addr, shutdown_signal(shutdown_on_stdin_close))
        .await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let log_level = cli.log_level.clone().unwrap_or_else(|| "info".to_owned());
    init_tracing(&log_level, cli.log_json);

    let enabled = parse_enabled_backends(cli.enabled_backends.as_deref())?;
    let base_lib_path = cli
        .lib_dir
        .as_deref()
        .unwrap_or(&Path::new("./resources/libs"));
    let llama_lib_dir = enabled.llama.then(|| base_lib_path.join("llama"));
    let whisper_lib_dir = enabled.whisper.then(|| base_lib_path.join("whisper"));
    let diffusion_lib_dir = enabled.diffusion.then(|| base_lib_path.join("diffusion"));

    slab_core::api::init(slab_core::api::Config {
        queue_capacity: cli.queue_capacity.unwrap_or(64),
        backend_capacity: cli.backend_capacity.unwrap_or(4),
        llama_lib_dir,
        whisper_lib_dir,
        diffusion_lib_dir,
    })
    .context("failed to initialize slab-core runtime")?;

    serve_grpc(&cli.grpc_bind, cli.shutdown_on_stdin_close).await?;
    info!("slab-runtime stopped");
    Ok(())
}
