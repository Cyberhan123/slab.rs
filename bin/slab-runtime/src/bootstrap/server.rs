use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};

use anyhow::Context;
use futures::StreamExt;
use slab_proto::slab::ipc::v1 as pb;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tonic::transport::server::Connected;
use tracing::info;

use crate::api::handlers::GrpcServiceImpl;

use super::signals;

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

pub(super) async fn serve_grpc(
    grpc_bind: &str,
    shutdown_on_stdin_close: bool,
    grpc_service: GrpcServiceImpl,
) -> anyhow::Result<()> {
    if let Some(raw_ipc_path) = grpc_bind.strip_prefix("ipc://") {
        let ipc_path = raw_ipc_path.trim();
        if ipc_path.is_empty() {
            anyhow::bail!("invalid IPC gRPC endpoint '{}': missing socket/pipe path", grpc_bind);
        }

        #[cfg(unix)]
        {
            if tokio::fs::try_exists(ipc_path).await.unwrap_or(false)
                && let Err(error) = tokio::fs::remove_file(ipc_path).await
            {
                tracing::warn!(path = %ipc_path, error = %error, "failed to remove stale IPC socket path before bind");
            }
        }

        info!(transport = "ipc", path = %ipc_path, "slab-runtime gRPC listening");
        let incoming = parity_tokio_ipc::Endpoint::new(ipc_path.to_owned())
            .incoming()
            .with_context(|| format!("failed to bind IPC endpoint '{ipc_path}'"))?
            .map(|stream| stream.map(IpcIo::new));

        tonic::transport::Server::builder()
            .add_service(pb::ggml_llama_service_server::GgmlLlamaServiceServer::new(
                grpc_service.clone(),
            ))
            .add_service(pb::ggml_whisper_service_server::GgmlWhisperServiceServer::new(
                grpc_service.clone(),
            ))
            .add_service(pb::ggml_diffusion_service_server::GgmlDiffusionServiceServer::new(
                grpc_service.clone(),
            ))
            .add_service(
                pb::candle_transformers_service_server::CandleTransformersServiceServer::new(
                    grpc_service.clone(),
                ),
            )
            .add_service(pb::candle_diffusion_service_server::CandleDiffusionServiceServer::new(
                grpc_service.clone(),
            ))
            .add_service(pb::onnx_service_server::OnnxServiceServer::new(grpc_service.clone()))
            .serve_with_incoming_shutdown(
                incoming,
                signals::shutdown_signal(shutdown_on_stdin_close),
            )
            .await?;
        info!(transport = "ipc", path = %ipc_path, "slab-runtime gRPC server stopped");
        return Ok(());
    }

    let addr = grpc_bind
        .parse()
        .with_context(|| format!("invalid TCP gRPC bind address '{grpc_bind}'"))?;
    info!(transport = "http", %addr, "slab-runtime gRPC listening");
    tonic::transport::Server::builder()
        .add_service(pb::ggml_llama_service_server::GgmlLlamaServiceServer::new(
            grpc_service.clone(),
        ))
        .add_service(pb::ggml_whisper_service_server::GgmlWhisperServiceServer::new(
            grpc_service.clone(),
        ))
        .add_service(pb::ggml_diffusion_service_server::GgmlDiffusionServiceServer::new(
            grpc_service.clone(),
        ))
        .add_service(pb::candle_transformers_service_server::CandleTransformersServiceServer::new(
            grpc_service.clone(),
        ))
        .add_service(pb::candle_diffusion_service_server::CandleDiffusionServiceServer::new(
            grpc_service.clone(),
        ))
        .add_service(pb::onnx_service_server::OnnxServiceServer::new(grpc_service))
        .serve_with_shutdown(addr, signals::shutdown_signal(shutdown_on_stdin_close))
        .await?;
    info!(transport = "http", %addr, "slab-runtime gRPC server stopped");
    Ok(())
}
