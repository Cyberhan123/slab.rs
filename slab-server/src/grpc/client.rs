use anyhow::Context;

use super::pb;

const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;

async fn connect(
    endpoint: &str,
) -> anyhow::Result<pb::backend_service_client::BackendServiceClient<tonic::transport::Channel>> {
    let url = format!("http://{endpoint}");
    let channel = tonic::transport::Channel::from_shared(url)
        .context("invalid gRPC endpoint URL")?
        .connect()
        .await
        .with_context(|| format!("failed to connect to backend gRPC endpoint: {endpoint}"))?;

    Ok(
        pb::backend_service_client::BackendServiceClient::new(channel)
            .max_decoding_message_size(MAX_MESSAGE_BYTES)
            .max_encoding_message_size(MAX_MESSAGE_BYTES),
    )
}

pub async fn chat(endpoint: &str, req: pb::ChatRequest) -> anyhow::Result<String> {
    let mut client = connect(endpoint).await?;
    let response = client.chat(req).await?;
    Ok(response.into_inner().text)
}

pub async fn chat_stream(
    endpoint: &str,
    req: pb::ChatRequest,
) -> anyhow::Result<tonic::Streaming<pb::ChatStreamChunk>> {
    let mut client = connect(endpoint).await?;
    let response = client.chat_stream(req).await?;
    Ok(response.into_inner())
}

pub async fn transcribe(endpoint: &str, path: String) -> anyhow::Result<String> {
    let mut client = connect(endpoint).await?;
    let response = client
        .transcribe(pb::TranscribeRequest { path })
        .await
        .with_context(|| format!("transcribe RPC failed on endpoint: {endpoint}"))?;
    Ok(response.into_inner().text)
}

pub async fn generate_image(endpoint: &str, req: pb::ImageRequest) -> anyhow::Result<Vec<u8>> {
    let mut client = connect(endpoint).await?;
    let response = client
        .generate_image(req)
        .await
        .with_context(|| format!("generate_image RPC failed on endpoint: {endpoint}"))?;
    Ok(response.into_inner().image)
}

pub async fn load_model(
    endpoint: &str,
    req: pb::ModelLoadRequest,
) -> anyhow::Result<pb::ModelStatusResponse> {
    let mut client = connect(endpoint).await?;
    let response = client
        .load_model(req)
        .await
        .with_context(|| format!("load_model RPC failed on endpoint: {endpoint}"))?;
    Ok(response.into_inner())
}

pub async fn unload_model(
    endpoint: &str,
    req: pb::ModelUnloadRequest,
) -> anyhow::Result<pb::ModelStatusResponse> {
    let mut client = connect(endpoint).await?;
    let response = client
        .unload_model(req)
        .await
        .with_context(|| format!("unload_model RPC failed on endpoint: {endpoint}"))?;
    Ok(response.into_inner())
}

pub async fn reload_library(
    endpoint: &str,
    req: pb::ReloadLibraryRequest,
) -> anyhow::Result<pb::ModelStatusResponse> {
    let mut client = connect(endpoint).await?;
    let response = client
        .reload_library(req)
        .await
        .with_context(|| format!("reload_library RPC failed on endpoint: {endpoint}"))?;
    Ok(response.into_inner())
}
