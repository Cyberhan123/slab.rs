use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::Context;
use hyper_util::rt::TokioIo;
use slab_types::RuntimeBackendId;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;
use tracing::warn;

use crate::config::Config;

const STRICT_CONNECT_ATTEMPTS: usize = 30;
const BEST_EFFORT_CONNECT_ATTEMPTS: usize = 3;
const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcGatewayConnectPolicy {
    Strict,
    BestEffort,
}

impl GrpcGatewayConnectPolicy {
    const fn connect_attempts(self) -> usize {
        match self {
            Self::Strict => STRICT_CONNECT_ATTEMPTS,
            Self::BestEffort => BEST_EFFORT_CONNECT_ATTEMPTS,
        }
    }
}

#[derive(Debug, Clone)]
enum GrpcEndpoint {
    Http(String),
    Ipc(String),
}

#[derive(Clone, Default)]
pub struct GrpcGateway {
    backend_channels: Arc<RwLock<HashMap<RuntimeBackendId, Channel>>>,
}

impl std::fmt::Debug for GrpcGateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let guard = self.backend_channels.read().unwrap_or_else(|error| error.into_inner());
        let mut backends: Vec<&str> = guard.keys().map(|backend| backend.canonical_id()).collect();
        backends.sort_unstable();
        f.debug_struct("GrpcGateway")
            .field("chat", &guard.contains_key(&RuntimeBackendId::GgmlLlama))
            .field("chat_stream", &guard.contains_key(&RuntimeBackendId::GgmlLlama))
            .field("transcribe", &guard.contains_key(&RuntimeBackendId::GgmlWhisper))
            .field("generate_image", &guard.contains_key(&RuntimeBackendId::GgmlDiffusion))
            .field("backends", &backends)
            .finish()
    }
}

impl GrpcGateway {
    pub async fn connect_from_config(config: &Config) -> anyhow::Result<Self> {
        let gateway = Self::default();
        gateway.refresh_from_config(config).await?;
        Ok(gateway)
    }

    pub async fn connect_from_config_best_effort(config: &Config) -> Self {
        let gateway = Self::default();
        let _ = gateway.refresh_from_config_best_effort(config).await;
        gateway
    }

    pub async fn refresh_from_config(&self, config: &Config) -> anyhow::Result<()> {
        self.refresh_from_config_with_policy(config, GrpcGatewayConnectPolicy::Strict).await
    }

    pub async fn refresh_from_config_best_effort(&self, config: &Config) -> anyhow::Result<()> {
        self.refresh_from_config_with_policy(config, GrpcGatewayConnectPolicy::BestEffort).await
    }

    async fn refresh_from_config_with_policy(
        &self,
        config: &Config,
        policy: GrpcGatewayConnectPolicy,
    ) -> anyhow::Result<()> {
        let mut refreshed_channels = HashMap::new();
        for (backend_id, endpoint) in [
            (RuntimeBackendId::GgmlLlama, config.llama_grpc_endpoint.as_deref()),
            (RuntimeBackendId::GgmlWhisper, config.whisper_grpc_endpoint.as_deref()),
            (RuntimeBackendId::GgmlDiffusion, config.diffusion_grpc_endpoint.as_deref()),
        ] {
            match connect_optional(endpoint, policy).await {
                Ok(Some(channel)) => {
                    refreshed_channels.insert(backend_id, channel);
                }
                Ok(None) => {}
                Err(error) if policy == GrpcGatewayConnectPolicy::BestEffort => {
                    warn!(
                        backend = backend_id.canonical_id(),
                        endpoint = %endpoint.unwrap_or(""),
                        error = %error,
                        "skipping unavailable gRPC backend during HTTP gateway bootstrap"
                    );
                }
                Err(error) => return Err(error),
            }
        }

        let mut guard = self.backend_channels.write().unwrap_or_else(|error| error.into_inner());
        *guard = refreshed_channels;
        Ok(())
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
        self.backend_channels
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .get(&backend_id)
            .cloned()
    }

    pub fn has_backend(&self, backend_id: RuntimeBackendId) -> bool {
        self.backend_channels
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .contains_key(&backend_id)
    }
}

async fn connect_optional(
    endpoint: Option<&str>,
    policy: GrpcGatewayConnectPolicy,
) -> anyhow::Result<Option<Channel>> {
    match endpoint {
        Some(endpoint) if !endpoint.trim().is_empty() => {
            Ok(Some(connect_channel(endpoint, policy.connect_attempts()).await?))
        }
        _ => Ok(None),
    }
}

async fn connect_channel(endpoint: &str, attempts: usize) -> anyhow::Result<Channel> {
    let endpoint = parse_grpc_endpoint(endpoint)?;
    let attempts = attempts.max(1);

    let mut last_error = None;
    for _ in 0..attempts {
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
