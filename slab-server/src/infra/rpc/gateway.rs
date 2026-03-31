use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use hyper_util::rt::TokioIo;
use slab_types::RuntimeBackendId;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;

use crate::config::Config;

const CONNECT_ATTEMPTS: usize = 30;
const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
enum GrpcEndpoint {
    Http(String),
    Ipc(String),
}

#[derive(Clone, Default)]
pub struct GrpcGateway {
    backend_channels: HashMap<RuntimeBackendId, Channel>,
}

impl std::fmt::Debug for GrpcGateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut backends: Vec<&str> =
            self.backend_channels.keys().map(|backend| backend.canonical_id()).collect();
        backends.sort_unstable();
        f.debug_struct("GrpcGateway")
            .field(
                "chat",
                &self.backend_channels.contains_key(&RuntimeBackendId::GgmlLlama),
            )
            .field(
                "chat_stream",
                &self.backend_channels.contains_key(&RuntimeBackendId::GgmlLlama),
            )
            .field(
                "transcribe",
                &self.backend_channels.contains_key(&RuntimeBackendId::GgmlWhisper),
            )
            .field(
                "generate_image",
                &self.backend_channels.contains_key(&RuntimeBackendId::GgmlDiffusion),
            )
            .field("backends", &backends)
            .finish()
    }
}

impl GrpcGateway {
    pub async fn connect_from_config(config: &Config) -> anyhow::Result<Self> {
        let mut gateway = Self::default();

        for (backend_id, endpoint) in [
            (RuntimeBackendId::GgmlLlama, config.llama_grpc_endpoint.as_deref()),
            (RuntimeBackendId::GgmlWhisper, config.whisper_grpc_endpoint.as_deref()),
            (RuntimeBackendId::GgmlDiffusion, config.diffusion_grpc_endpoint.as_deref()),
        ] {
            if let Some(channel) = connect_optional(endpoint).await? {
                gateway.backend_channels.insert(backend_id, channel);
            }
        }

        Ok(gateway)
    }

    pub fn chat_channel(&self) -> Option<Channel> {
        self.backend_channel(RuntimeBackendId::GgmlLlama)
    }

    pub fn transcribe_channel(&self) -> Option<Channel> {
        self.backend_channel(RuntimeBackendId::GgmlWhisper)
    }

    pub fn generate_image_channel(&self) -> Option<Channel> {
        self.backend_channel(RuntimeBackendId::GgmlDiffusion)
    }

    pub fn backend_channel(&self, backend_id: RuntimeBackendId) -> Option<Channel> {
        self.backend_channels.get(&backend_id).cloned()
    }

    pub fn has_backend(&self, backend_id: RuntimeBackendId) -> bool {
        self.backend_channels.contains_key(&backend_id)
    }
}

async fn connect_optional(endpoint: Option<&str>) -> anyhow::Result<Option<Channel>> {
    match endpoint {
        Some(endpoint) if !endpoint.trim().is_empty() => Ok(Some(connect_channel(endpoint).await?)),
        _ => Ok(None),
    }
}

async fn connect_channel(endpoint: &str) -> anyhow::Result<Channel> {
    let endpoint = parse_grpc_endpoint(endpoint)?;

    let mut last_error = None;
    for _ in 0..CONNECT_ATTEMPTS {
        let connect_result = match &endpoint {
            GrpcEndpoint::Http(url) => connect_http_channel(url).await,
            GrpcEndpoint::Ipc(path) => connect_ipc_channel(path).await,
        };

        match connect_result {
            Ok(channel) => return Ok(channel),
            Err(err) => {
                last_error = Some(err);
                tokio::time::sleep(CONNECT_RETRY_DELAY).await;
            }
        }
    }

    let err =
        last_error.map(|e| e.to_string()).unwrap_or_else(|| "unknown connection error".to_string());
    anyhow::bail!("failed to connect to gRPC endpoint {}: {err}", endpoint.as_display());
}

impl GrpcEndpoint {
    fn as_display(&self) -> &str {
        match self {
            Self::Http(url) => url.as_str(),
            Self::Ipc(path) => path.as_str(),
        }
    }
}

fn parse_grpc_endpoint(raw: &str) -> anyhow::Result<GrpcEndpoint> {
    let raw = raw.trim();
    if raw.is_empty() {
        anyhow::bail!("gRPC endpoint is empty");
    }

    if let Some(path) = raw.strip_prefix("ipc://") {
        let path = path.trim();
        if path.is_empty() {
            anyhow::bail!("invalid IPC endpoint '{}': missing socket/pipe path", raw);
        }
        return Ok(GrpcEndpoint::Ipc(path.to_owned()));
    }

    let url = if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.to_owned()
    } else {
        format!("http://{raw}")
    };

    Ok(GrpcEndpoint::Http(url))
}

async fn connect_http_channel(url: &str) -> anyhow::Result<Channel> {
    let transport = Endpoint::from_shared(url.to_owned())
        .with_context(|| format!("invalid gRPC URL: {url}"))?;
    let channel = transport
        .connect()
        .await
        .with_context(|| format!("failed to connect to HTTP gRPC endpoint '{url}'"))?;
    Ok(channel)
}

async fn connect_ipc_channel(path: &str) -> anyhow::Result<Channel> {
    let path_display = path.to_owned();
    let path_for_connector = path_display.clone();

    let channel = Endpoint::from_static("http://[::]:50051")
        .connect_with_connector(service_fn(move |_| {
            let path = path_for_connector.clone();
            async move {
                let conn = parity_tokio_ipc::Endpoint::connect(path).await?;
                Ok::<_, std::io::Error>(TokioIo::new(conn))
            }
        }))
        .await
        .with_context(|| format!("failed to connect to IPC gRPC endpoint '{path_display}'"))?;
    Ok(channel)
}
