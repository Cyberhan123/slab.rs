use anyhow::Context;
use tonic::service::Interceptor;
use tonic::transport::Channel;
use tracing::{debug, warn};
use uuid::Uuid;

use super::pb;

const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Request-ID interceptor
// ---------------------------------------------------------------------------

/// Client-side gRPC interceptor that generates a fresh UUID per RPC call and
/// injects it as the `x-request-id` metadata header so that every outbound
/// request can be correlated end-to-end across the server/runtime boundary.
pub struct RequestIdInterceptor {
    request_id: String,
}

impl RequestIdInterceptor {
    /// Create an interceptor with a newly generated request ID.
    pub fn new() -> Self {
        let request_id = Uuid::new_v4().to_string();
        debug!(request_id = %request_id, "created new gRPC request interceptor");
        Self { request_id }
    }

    /// Return the request ID that will be (or has been) injected into request
    /// metadata.
    pub fn id(&self) -> &str {
        &self.request_id
    }
}

impl Interceptor for RequestIdInterceptor {
    fn call(&mut self, mut req: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        match tonic::metadata::MetadataValue::try_from(self.request_id.as_str()) {
            Ok(v) => {
                req.metadata_mut().insert("x-request-id", v);
            }
            Err(e) => {
                warn!(
                    request_id = %self.request_id,
                    error = %e,
                    "failed to encode x-request-id as gRPC metadata value"
                );
            }
        }
        Ok(req)
    }
}

// ---------------------------------------------------------------------------
// Typed client helpers
// ---------------------------------------------------------------------------

type LlamaClient = pb::llama_service_client::LlamaServiceClient<
    tonic::service::interceptor::InterceptedService<Channel, RequestIdInterceptor>,
>;
type WhisperClient = pb::whisper_service_client::WhisperServiceClient<
    tonic::service::interceptor::InterceptedService<Channel, RequestIdInterceptor>,
>;
type DiffusionClient = pb::diffusion_service_client::DiffusionServiceClient<
    tonic::service::interceptor::InterceptedService<Channel, RequestIdInterceptor>,
>;

/// Create a Llama client wrapped with a fresh [`RequestIdInterceptor`].
/// Returns both the client and the generated request ID for downstream
/// logging.
fn llama_client(channel: Channel) -> (LlamaClient, String) {
    let interceptor = RequestIdInterceptor::new();
    let request_id = interceptor.id().to_owned();
    let client =
        pb::llama_service_client::LlamaServiceClient::with_interceptor(channel, interceptor)
            .max_decoding_message_size(MAX_MESSAGE_BYTES)
            .max_encoding_message_size(MAX_MESSAGE_BYTES);
    (client, request_id)
}

/// Create a Whisper client wrapped with a fresh [`RequestIdInterceptor`].
fn whisper_client(channel: Channel) -> (WhisperClient, String) {
    let interceptor = RequestIdInterceptor::new();
    let request_id = interceptor.id().to_owned();
    let client =
        pb::whisper_service_client::WhisperServiceClient::with_interceptor(channel, interceptor)
            .max_decoding_message_size(MAX_MESSAGE_BYTES)
            .max_encoding_message_size(MAX_MESSAGE_BYTES);
    (client, request_id)
}

/// Create a Diffusion client wrapped with a fresh [`RequestIdInterceptor`].
fn diffusion_client(channel: Channel) -> (DiffusionClient, String) {
    let interceptor = RequestIdInterceptor::new();
    let request_id = interceptor.id().to_owned();
    let client = pb::diffusion_service_client::DiffusionServiceClient::with_interceptor(
        channel,
        interceptor,
    )
    .max_decoding_message_size(MAX_MESSAGE_BYTES)
    .max_encoding_message_size(MAX_MESSAGE_BYTES);
    (client, request_id)
}

// ---------------------------------------------------------------------------
// Backend kind
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Public RPC call wrappers
// ---------------------------------------------------------------------------

/// Log a downstream gRPC failure and return the status as an error.
///
/// Called in every RPC wrapper so that `status.code()` and
/// `status.message()` are always recorded in the structured log even when
/// the caller discards the error detail.
#[inline]
fn log_grpc_error(rpc: &str, request_id: &str, status: &tonic::Status) {
    warn!(
        rpc,
        request_id,
        grpc.code = %status.code(),
        grpc.message = %status.message(),
        "downstream gRPC call failed"
    );
}

pub async fn chat(channel: Channel, req: pb::ChatRequest) -> anyhow::Result<String> {
    let (mut client, request_id) = llama_client(channel);
    debug!(request_id = %request_id, "sending gRPC chat request");
    let response =
        client.chat(req).await.inspect_err(|s| log_grpc_error("chat", &request_id, s))?;
    Ok(response.into_inner().text)
}

pub async fn chat_stream(
    channel: Channel,
    req: pb::ChatRequest,
) -> anyhow::Result<tonic::Streaming<pb::ChatStreamChunk>> {
    let (mut client, request_id) = llama_client(channel);
    debug!(request_id = %request_id, "sending gRPC chat_stream request");
    let response = client
        .chat_stream(req)
        .await
        .inspect_err(|s| log_grpc_error("chat_stream", &request_id, s))?;
    Ok(response.into_inner())
}

pub async fn transcribe(channel: Channel, req: pb::TranscribeRequest) -> anyhow::Result<String> {
    let (mut client, request_id) = whisper_client(channel);
    let vad_enabled = req.vad.as_ref().is_some_and(|v| v.enabled);
    let decode_configured = req.decode.is_some();
    debug!(
        request_id = %request_id,
        audio_path = %req.path,
        vad_enabled,
        decode_configured,
        "sending gRPC transcribe request"
    );
    let response = client
        .transcribe(req)
        .await
        .inspect_err(|s| log_grpc_error("transcribe", &request_id, s))
        .context("transcribe RPC failed")?;
    Ok(response.into_inner().text)
}

pub async fn generate_image(channel: Channel, req: pb::ImageRequest) -> anyhow::Result<Vec<u8>> {
    let (mut client, request_id) = diffusion_client(channel);
    debug!(request_id = %request_id, "sending gRPC generate_image request");
    let response = client
        .generate_image(req)
        .await
        .inspect_err(|s| log_grpc_error("generate_image", &request_id, s))
        .context("generate_image RPC failed")?;
    Ok(response.into_inner().images_json)
}

pub async fn generate_video(channel: Channel, req: pb::VideoRequest) -> anyhow::Result<Vec<u8>> {
    let (mut client, request_id) = diffusion_client(channel);
    debug!(request_id = %request_id, "sending gRPC generate_video request");
    let response = client
        .generate_video(req)
        .await
        .inspect_err(|s| log_grpc_error("generate_video", &request_id, s))
        .context("generate_video RPC failed")?;
    Ok(response.into_inner().frames_json)
}

// ---------------------------------------------------------------------------
// Helper macro: creates a client, makes one call, logs any gRPC failure, and
// returns `(Result<tonic::Response<R>, tonic::Status>, String)` so callers
// can attach the request_id to the anyhow context without repeating the
// log-and-propagate boilerplate for every backend branch.
// ---------------------------------------------------------------------------
macro_rules! grpc_call {
    ($rpc:literal, $client_fn:ident, $channel:expr, $method:ident, $req:expr) => {{
        let (mut client, request_id) = $client_fn($channel);
        let result = client.$method($req).await.map_err(|s: tonic::Status| {
            log_grpc_error($rpc, &request_id, &s);
            s
        });
        (result, request_id)
    }};
}

pub async fn load_model(
    channel: Channel,
    backend_id: &str,
    req: pb::ModelLoadRequest,
) -> anyhow::Result<pb::ModelStatusResponse> {
    let backend = BackendKind::parse(backend_id)?;
    debug!(backend_id, model_path = %req.model_path, "sending gRPC load_model request");

    let (response, request_id) = match backend {
        BackendKind::Llama => grpc_call!("load_model", llama_client, channel, load_model, req),
        BackendKind::Whisper => grpc_call!("load_model", whisper_client, channel, load_model, req),
        BackendKind::Diffusion => {
            grpc_call!("load_model", diffusion_client, channel, load_model, req)
        }
    };

    let response = response.with_context(|| {
        format!("load_model RPC failed for backend: {backend_id} (request_id={request_id})")
    })?;
    Ok(response.into_inner())
}

pub async fn unload_model(
    channel: Channel,
    backend_id: &str,
    req: pb::ModelUnloadRequest,
) -> anyhow::Result<pb::ModelStatusResponse> {
    let backend = BackendKind::parse(backend_id)?;
    debug!(backend_id, "sending gRPC unload_model request");

    let (response, request_id) = match backend {
        BackendKind::Llama => grpc_call!("unload_model", llama_client, channel, unload_model, req),
        BackendKind::Whisper => {
            grpc_call!("unload_model", whisper_client, channel, unload_model, req)
        }
        BackendKind::Diffusion => {
            grpc_call!("unload_model", diffusion_client, channel, unload_model, req)
        }
    };

    let response = response.with_context(|| {
        format!("unload_model RPC failed for backend: {backend_id} (request_id={request_id})")
    })?;
    Ok(response.into_inner())
}

pub async fn reload_library(
    channel: Channel,
    backend_id: &str,
    req: pb::ReloadLibraryRequest,
) -> anyhow::Result<pb::ModelStatusResponse> {
    let backend = BackendKind::parse(backend_id)?;
    debug!(backend_id, lib_path = %req.lib_path, "sending gRPC reload_library request");

    let (response, request_id) = match backend {
        BackendKind::Llama => {
            grpc_call!("reload_library", llama_client, channel, reload_library, req)
        }
        BackendKind::Whisper => {
            grpc_call!("reload_library", whisper_client, channel, reload_library, req)
        }
        BackendKind::Diffusion => {
            grpc_call!("reload_library", diffusion_client, channel, reload_library, req)
        }
    };

    let response = response.with_context(|| {
        format!("reload_library RPC failed for backend: {backend_id} (request_id={request_id})")
    })?;
    Ok(response.into_inner())
}
