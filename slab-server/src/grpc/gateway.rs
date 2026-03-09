use anyhow::Context;
use tonic::Streaming;
use tower::service_fn;
use tower::util::BoxCloneService;

use crate::config::Config;

use super::{client, pb};

pub type ChatService = BoxCloneService<pb::ChatRequest, pb::ChatResponse, anyhow::Error>;
pub type ChatStreamService =
    BoxCloneService<pb::ChatRequest, Streaming<pb::ChatStreamChunk>, anyhow::Error>;
pub type TranscribeService =
    BoxCloneService<pb::TranscribeRequest, pb::TranscribeResponse, anyhow::Error>;
pub type GenerateImageService = BoxCloneService<pb::ImageRequest, pb::ImageResponse, anyhow::Error>;
pub type LoadModelService =
    BoxCloneService<pb::ModelLoadRequest, pb::ModelStatusResponse, anyhow::Error>;
pub type UnloadModelService =
    BoxCloneService<pb::ModelUnloadRequest, pb::ModelStatusResponse, anyhow::Error>;
pub type ReloadLibraryService =
    BoxCloneService<pb::ReloadLibraryRequest, pb::ModelStatusResponse, anyhow::Error>;

pub fn chat(endpoint: impl Into<String>) -> ChatService {
    let endpoint = endpoint.into();
    BoxCloneService::new(service_fn(move |request: pb::ChatRequest| {
        let endpoint = endpoint.clone();
        async move {
            let text = client::chat(&endpoint, request).await?;
            Ok(pb::ChatResponse { text })
        }
    }))
}

pub fn chat_stream(endpoint: impl Into<String>) -> ChatStreamService {
    let endpoint = endpoint.into();
    BoxCloneService::new(service_fn(move |request: pb::ChatRequest| {
        let endpoint = endpoint.clone();
        async move { client::chat_stream(&endpoint, request).await }
    }))
}

pub fn transcribe(endpoint: impl Into<String>) -> TranscribeService {
    let endpoint = endpoint.into();
    BoxCloneService::new(service_fn(move |request: pb::TranscribeRequest| {
        let endpoint = endpoint.clone();
        async move {
            let text = client::transcribe(&endpoint, request.path).await?;
            Ok(pb::TranscribeResponse { text })
        }
    }))
}

pub fn generate_image(endpoint: impl Into<String>) -> GenerateImageService {
    let endpoint = endpoint.into();
    BoxCloneService::new(service_fn(move |request: pb::ImageRequest| {
        let endpoint = endpoint.clone();
        async move {
            let image = client::generate_image(&endpoint, request).await?;
            Ok(pb::ImageResponse { image })
        }
    }))
}

pub fn load_model(endpoint: impl Into<String>) -> LoadModelService {
    let endpoint = endpoint.into();
    BoxCloneService::new(service_fn(move |request: pb::ModelLoadRequest| {
        let endpoint = endpoint.clone();
        async move { client::load_model(&endpoint, request).await }
    }))
}

pub fn unload_model(endpoint: impl Into<String>) -> UnloadModelService {
    let endpoint = endpoint.into();
    BoxCloneService::new(service_fn(move |request: pb::ModelUnloadRequest| {
        let endpoint = endpoint.clone();
        async move { client::unload_model(&endpoint, request).await }
    }))
}

pub fn reload_library(endpoint: impl Into<String>) -> ReloadLibraryService {
    let endpoint = endpoint.into();
    BoxCloneService::new(service_fn(move |request: pb::ReloadLibraryRequest| {
        let endpoint = endpoint.clone();
        async move { client::reload_library(&endpoint, request).await }
    }))
}

pub fn map_via_serde<TSrc, TDst>(value: &TSrc) -> anyhow::Result<TDst>
where
    TSrc: serde::Serialize,
    TDst: for<'de> serde::Deserialize<'de>,
{
    let json =
        serde_json::to_value(value).context("failed to serialize request mapping payload")?;
    serde_json::from_value(json).context("failed to deserialize mapped request payload")
}

pub fn endpoint_for_backend(config: &Config, backend_id: &str) -> Option<String> {
    match backend_id.trim().to_ascii_lowercase().as_str() {
        "ggml.llama" | "llama" => config.llama_grpc_endpoint.clone(),
        "ggml.whisper" | "whisper" => config.whisper_grpc_endpoint.clone(),
        "ggml.diffusion" | "diffusion" => config.diffusion_grpc_endpoint.clone(),
        _ => None,
    }
}
