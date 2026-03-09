use anyhow::Context;
use tonic::transport::Channel;

use super::pb;

const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;

fn llama_client(channel: Channel) -> pb::llama_service_client::LlamaServiceClient<Channel> {
    pb::llama_service_client::LlamaServiceClient::new(channel)
        .max_decoding_message_size(MAX_MESSAGE_BYTES)
        .max_encoding_message_size(MAX_MESSAGE_BYTES)
}

fn whisper_client(channel: Channel) -> pb::whisper_service_client::WhisperServiceClient<Channel> {
    pb::whisper_service_client::WhisperServiceClient::new(channel)
        .max_decoding_message_size(MAX_MESSAGE_BYTES)
        .max_encoding_message_size(MAX_MESSAGE_BYTES)
}

fn diffusion_client(
    channel: Channel,
) -> pb::diffusion_service_client::DiffusionServiceClient<Channel> {
    pb::diffusion_service_client::DiffusionServiceClient::new(channel)
        .max_decoding_message_size(MAX_MESSAGE_BYTES)
        .max_encoding_message_size(MAX_MESSAGE_BYTES)
}

#[derive(Clone, Copy)]
enum BackendKind {
    Llama,
    Whisper,
    Diffusion,
}

impl BackendKind {
    fn parse(raw: &str) -> anyhow::Result<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "ggml.llama" | "llama" => Ok(Self::Llama),
            "ggml.whisper" | "whisper" => Ok(Self::Whisper),
            "ggml.diffusion" | "diffusion" => Ok(Self::Diffusion),
            _ => anyhow::bail!("unknown backend_id: {raw}"),
        }
    }
}

pub async fn chat(channel: Channel, req: pb::ChatRequest) -> anyhow::Result<String> {
    let mut client = llama_client(channel);
    let response = client.chat(req).await?;
    Ok(response.into_inner().text)
}

pub async fn chat_stream(
    channel: Channel,
    req: pb::ChatRequest,
) -> anyhow::Result<tonic::Streaming<pb::ChatStreamChunk>> {
    let mut client = llama_client(channel);
    let response = client.chat_stream(req).await?;
    Ok(response.into_inner())
}

pub async fn transcribe(channel: Channel, path: String) -> anyhow::Result<String> {
    let mut client = whisper_client(channel);
    let response = client
        .transcribe(pb::TranscribeRequest { path })
        .await
        .context("transcribe RPC failed")?;
    Ok(response.into_inner().text)
}

pub async fn generate_image(channel: Channel, req: pb::ImageRequest) -> anyhow::Result<Vec<u8>> {
    let mut client = diffusion_client(channel);
    let response = client
        .generate_image(req)
        .await
        .context("generate_image RPC failed")?;
    Ok(response.into_inner().image)
}

pub async fn load_model(
    channel: Channel,
    backend_id: &str,
    req: pb::ModelLoadRequest,
) -> anyhow::Result<pb::ModelStatusResponse> {
    let backend = BackendKind::parse(backend_id)?;
    let response = match backend {
        BackendKind::Llama => llama_client(channel).load_model(req).await,
        BackendKind::Whisper => whisper_client(channel).load_model(req).await,
        BackendKind::Diffusion => diffusion_client(channel).load_model(req).await,
    }
    .with_context(|| format!("load_model RPC failed for backend: {backend_id}"))?;

    Ok(response.into_inner())
}

pub async fn unload_model(
    channel: Channel,
    backend_id: &str,
    req: pb::ModelUnloadRequest,
) -> anyhow::Result<pb::ModelStatusResponse> {
    let backend = BackendKind::parse(backend_id)?;
    let response = match backend {
        BackendKind::Llama => llama_client(channel).unload_model(req).await,
        BackendKind::Whisper => whisper_client(channel).unload_model(req).await,
        BackendKind::Diffusion => diffusion_client(channel).unload_model(req).await,
    }
    .with_context(|| format!("unload_model RPC failed for backend: {backend_id}"))?;

    Ok(response.into_inner())
}

pub async fn reload_library(
    channel: Channel,
    backend_id: &str,
    req: pb::ReloadLibraryRequest,
) -> anyhow::Result<pb::ModelStatusResponse> {
    let backend = BackendKind::parse(backend_id)?;
    let response = match backend {
        BackendKind::Llama => llama_client(channel).reload_library(req).await,
        BackendKind::Whisper => whisper_client(channel).reload_library(req).await,
        BackendKind::Diffusion => diffusion_client(channel).reload_library(req).await,
    }
    .with_context(|| format!("reload_library RPC failed for backend: {backend_id}"))?;

    Ok(response.into_inner())
}
