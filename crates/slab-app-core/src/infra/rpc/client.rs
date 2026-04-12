use std::time::Duration;

use anyhow::Context;
use slab_types::RuntimeBackendId;
use tonic::service::Interceptor;
use tonic::transport::Channel;
use tracing::{debug, warn};
use uuid::Uuid;

use super::pb;

const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;
const LOAD_MODEL_MAX_ATTEMPTS: usize = 3;
const LOAD_MODEL_RETRY_DELAY: Duration = Duration::from_millis(250);

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
    fn from_backend_id(value: RuntimeBackendId) -> anyhow::Result<Self> {
        match value {
            RuntimeBackendId::GgmlLlama => Ok(Self::Llama),
            RuntimeBackendId::GgmlWhisper => Ok(Self::Whisper),
            RuntimeBackendId::GgmlDiffusion => Ok(Self::Diffusion),
            other => anyhow::bail!("backend {} is not served by the gRPC runtime", other),
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
    let status_message = status.message();
    let is_transport_disconnect = status.code() == tonic::Code::Unknown
        && (status_message.contains("transport error")
            || status_message.contains("broken pipe")
            || status_message.contains("connection error"));

    warn!(
        rpc,
        request_id,
        grpc.code = %status.code(),
        grpc.message = %status_message,
        grpc.transport_disconnect = is_transport_disconnect,
        "downstream gRPC call failed"
    );
}

fn grpc_status_to_anyhow(rpc: &str, request_id: &str, status: tonic::Status) -> anyhow::Error {
    let code = status.code();
    let message = status.message().to_owned();
    anyhow::Error::from(status).context(format!(
        "{rpc} RPC failed (request_id={request_id}, code={code}, message={message})"
    ))
}

pub fn is_transient_runtime_status(status: &tonic::Status) -> bool {
    let message = status.message();
    matches!(status.code(), tonic::Code::Unavailable)
        || (matches!(status.code(), tonic::Code::Unknown)
            && (message.contains("transport error")
                || message.contains("broken pipe")
                || message.contains("connection error")
                || message.contains("os error 2")))
}

pub fn transient_runtime_detail(err: &anyhow::Error) -> Option<String> {
    let status = err.chain().find_map(|cause| cause.downcast_ref::<tonic::Status>())?;
    is_transient_runtime_status(status).then(|| {
        let message = status.message().trim();
        if message.is_empty() { status.to_string() } else { message.to_owned() }
    })
}

pub async fn chat(channel: Channel, req: pb::ChatRequest) -> anyhow::Result<pb::ChatResponse> {
    let (mut client, request_id) = llama_client(channel);
    debug!(request_id = %request_id, "sending gRPC chat request");
    let response =
        client.chat(req).await.inspect_err(|s| log_grpc_error("chat", &request_id, s))?;
    Ok(response.into_inner())
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
    let response = client.transcribe(req).await.map_err(|status| {
        log_grpc_error("transcribe", &request_id, &status);
        grpc_status_to_anyhow("transcribe", &request_id, status)
    })?;
    Ok(response.into_inner().text)
}

pub async fn generate_image(channel: Channel, req: pb::ImageRequest) -> anyhow::Result<Vec<u8>> {
    let (mut client, request_id) = diffusion_client(channel);
    debug!(request_id = %request_id, "sending gRPC generate_image request");
    let response = client.generate_image(req).await.map_err(|status| {
        log_grpc_error("generate_image", &request_id, &status);
        grpc_status_to_anyhow("generate_image", &request_id, status)
    })?;
    Ok(response.into_inner().images_json)
}

pub async fn generate_video(channel: Channel, req: pb::VideoRequest) -> anyhow::Result<Vec<u8>> {
    let (mut client, request_id) = diffusion_client(channel);
    debug!(request_id = %request_id, "sending gRPC generate_video request");
    let response = client.generate_video(req).await.map_err(|status| {
        log_grpc_error("generate_video", &request_id, &status);
        grpc_status_to_anyhow("generate_video", &request_id, status)
    })?;
    Ok(response.into_inner().frames_json)
}

// ---------------------------------------------------------------------------
// Helper macro: creates a client, makes one call, logs any gRPC failure, and
// returns `(Result<tonic::Response<R>, tonic::Status>, String)` so callers
// can attach the request_id to the anyhow context without repeating the
// log-and-propagate boilerplate for every backend branch.
// ---------------------------------------------------------------------------
macro_rules! grpc_call {
    ($rpc:literal, $client_fn:ident, $channel:expr_2021, $method:ident, $req:expr_2021) => {{
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
    backend_id: RuntimeBackendId,
    req: pb::ModelLoadRequest,
) -> anyhow::Result<pb::ModelStatusResponse> {
    let backend = BackendKind::from_backend_id(backend_id)?;
    let model_path =
        req.common.as_ref().map(|common| common.model_path.as_str()).unwrap_or_default();
    let has_diffusion_overrides = match req.backend_params.as_ref() {
        Some(pb::model_load_request::BackendParams::GgmlDiffusion(params)) => {
            params.diffusion_model_path.is_some()
                || params.vae_path.is_some()
                || params.taesd_path.is_some()
                || params.clip_l_path.is_some()
                || params.clip_g_path.is_some()
                || params.t5xxl_path.is_some()
                || params.clip_vision_path.is_some()
                || params.control_net_path.is_some()
                || params.vae_device.is_some()
                || params.clip_device.is_some()
                || params.flash_attn
                || params.offload_params_to_cpu
                || params.enable_mmap
                || params.n_threads.is_some()
        }
        _ => false,
    };
    let (num_workers, context_length) = match req.backend_params.as_ref() {
        Some(pb::model_load_request::BackendParams::GgmlLlama(params)) => {
            (params.num_workers, params.context_length.unwrap_or_default())
        }
        _ => (0, 0),
    };

    debug!(
        backend = %backend_id,
        model_path = %model_path,
        num_workers,
        context_length,
        has_diffusion_overrides,
        "sending gRPC load_model request"
    );

    for attempt in 1..=LOAD_MODEL_MAX_ATTEMPTS {
        let (response, request_id) = match backend {
            BackendKind::Llama => {
                grpc_call!("load_model", llama_client, channel.clone(), load_model, req.clone())
            }
            BackendKind::Whisper => {
                grpc_call!("load_model", whisper_client, channel.clone(), load_model, req.clone())
            }
            BackendKind::Diffusion => {
                grpc_call!("load_model", diffusion_client, channel.clone(), load_model, req.clone())
            }
        };

        match response {
            Ok(response) => {
                debug!(
                    backend = %backend_id,
                    request_id = %request_id,
                    attempt,
                    status = %response.get_ref().status,
                    "gRPC load_model request completed"
                );
                return Ok(response.into_inner());
            }
            Err(status) => {
                let retryable = is_transient_runtime_status(&status);
                if retryable && attempt < LOAD_MODEL_MAX_ATTEMPTS {
                    warn!(
                        backend = %backend_id,
                        request_id = %request_id,
                        attempt,
                        max_attempts = LOAD_MODEL_MAX_ATTEMPTS,
                        grpc.code = %status.code(),
                        grpc.message = %status.message(),
                        retry_delay_ms = LOAD_MODEL_RETRY_DELAY.as_millis(),
                        "gRPC load_model failed with transient transport error; retrying"
                    );
                    tokio::time::sleep(LOAD_MODEL_RETRY_DELAY).await;
                    continue;
                }

                return Err(anyhow::Error::from(status).context(format!(
                    "load_model RPC failed for backend: {backend_id} (request_id={request_id}, attempt={attempt})"
                )));
            }
        }
    }

    unreachable!("load_model retry loop should always return")
}

pub async fn unload_model(
    channel: Channel,
    backend_id: RuntimeBackendId,
    req: pb::ModelUnloadRequest,
) -> anyhow::Result<pb::ModelStatusResponse> {
    let backend = BackendKind::from_backend_id(backend_id)?;
    debug!(backend = %backend_id, "sending gRPC unload_model request");

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
