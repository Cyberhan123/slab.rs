use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use tonic::transport::{Channel, Endpoint};

use crate::config::Config;

const CONNECT_ATTEMPTS: usize = 30;
const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(100);

const BACKEND_LLAMA: &str = "ggml.llama";
const BACKEND_WHISPER: &str = "ggml.whisper";
const BACKEND_DIFFUSION: &str = "ggml.diffusion";

#[derive(Clone, Default)]
pub struct GrpcGateway {
    backend_channels: HashMap<String, Channel>,
}

impl std::fmt::Debug for GrpcGateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut backends: Vec<&str> = self.backend_channels.keys().map(String::as_str).collect();
        backends.sort_unstable();
        f.debug_struct("GrpcGateway")
            .field("chat", &self.backend_channels.contains_key(BACKEND_LLAMA))
            .field(
                "chat_stream",
                &self.backend_channels.contains_key(BACKEND_LLAMA),
            )
            .field(
                "transcribe",
                &self.backend_channels.contains_key(BACKEND_WHISPER),
            )
            .field(
                "generate_image",
                &self.backend_channels.contains_key(BACKEND_DIFFUSION),
            )
            .field("backends", &backends)
            .finish()
    }
}

impl GrpcGateway {
    pub async fn connect_from_config(config: &Config) -> anyhow::Result<Self> {
        let mut gateway = Self::default();

        for (backend_id, endpoint) in [
            (BACKEND_LLAMA, config.llama_grpc_endpoint.as_deref()),
            (BACKEND_WHISPER, config.whisper_grpc_endpoint.as_deref()),
            (BACKEND_DIFFUSION, config.diffusion_grpc_endpoint.as_deref()),
        ] {
            if let Some(channel) = connect_optional(endpoint).await? {
                gateway
                    .backend_channels
                    .insert(backend_id.to_string(), channel);
            }
        }

        Ok(gateway)
    }

    pub fn chat_channel(&self) -> Option<Channel> {
        self.backend_channel(BACKEND_LLAMA)
    }

    pub fn transcribe_channel(&self) -> Option<Channel> {
        self.backend_channel(BACKEND_WHISPER)
    }

    pub fn generate_image_channel(&self) -> Option<Channel> {
        self.backend_channel(BACKEND_DIFFUSION)
    }

    pub fn backend_channel(&self, backend_id: &str) -> Option<Channel> {
        let key = canonical_backend_id(backend_id)?;
        self.backend_channels.get(key).cloned()
    }

    pub fn has_backend(&self, backend_id: &str) -> bool {
        canonical_backend_id(backend_id).is_some_and(|key| self.backend_channels.contains_key(key))
    }
}

async fn connect_optional(endpoint: Option<&str>) -> anyhow::Result<Option<Channel>> {
    match endpoint {
        Some(endpoint) if !endpoint.trim().is_empty() => Ok(Some(connect_channel(endpoint).await?)),
        _ => Ok(None),
    }
}

async fn connect_channel(endpoint: &str) -> anyhow::Result<Channel> {
    let url = format!("http://{endpoint}");
    let transport = Endpoint::from_shared(url.clone())
        .with_context(|| format!("invalid gRPC endpoint URL: {url}"))?;

    let mut last_error = None;
    for _ in 0..CONNECT_ATTEMPTS {
        match transport.clone().connect().await {
            Ok(channel) => return Ok(channel),
            Err(err) => {
                last_error = Some(err);
                tokio::time::sleep(CONNECT_RETRY_DELAY).await;
            }
        }
    }

    let err = last_error
        .map(|e| e.to_string())
        .unwrap_or_else(|| "unknown connection error".to_string());
    anyhow::bail!("failed to connect to gRPC endpoint {endpoint}: {err}");
}

fn canonical_backend_id(backend_id: &str) -> Option<&'static str> {
    match backend_id.trim().to_ascii_lowercase().as_str() {
        "ggml.llama" | "llama" => Some(BACKEND_LLAMA),
        "ggml.whisper" | "whisper" => Some(BACKEND_WHISPER),
        "ggml.diffusion" | "diffusion" => Some(BACKEND_DIFFUSION),
        _ => None,
    }
}
