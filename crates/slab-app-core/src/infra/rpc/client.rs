use std::future::Future;
use std::time::Duration;

use slab_types::RuntimeBackendId;
use tonic::service::Interceptor;
use tonic::transport::Channel;
use tracing::{debug, warn};
use uuid::Uuid;

use super::codec::ModelLoadRpcRequest;
use super::pb;

const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;
const LOAD_MODEL_MAX_ATTEMPTS: usize = 3;
const LOAD_MODEL_RETRY_DELAY: Duration = Duration::from_millis(250);
const UNARY_RPC_MAX_ATTEMPTS: usize = 2;
const UNARY_RPC_RETRY_DELAY: Duration = Duration::from_millis(100);
const RPC_REQUEST_TIMEOUT: Duration = Duration::from_secs(30 * 60);

pub struct RequestIdInterceptor {
    request_id: String,
}

impl RequestIdInterceptor {
    pub fn new() -> Self {
        let request_id = Uuid::new_v4().to_string();
        debug!(request_id = %request_id, "created new gRPC request interceptor");
        Self { request_id }
    }

    pub fn id(&self) -> &str {
        &self.request_id
    }
}

impl Default for RequestIdInterceptor {
    fn default() -> Self {
        Self::new()
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

type InterceptedChannel =
    tonic::service::interceptor::InterceptedService<Channel, RequestIdInterceptor>;
type GgmlLlamaClient = pb::ggml_llama_service_client::GgmlLlamaServiceClient<InterceptedChannel>;
type GgmlWhisperClient =
    pb::ggml_whisper_service_client::GgmlWhisperServiceClient<InterceptedChannel>;
type GgmlDiffusionClient =
    pb::ggml_diffusion_service_client::GgmlDiffusionServiceClient<InterceptedChannel>;
type CandleTransformersClient =
    pb::candle_transformers_service_client::CandleTransformersServiceClient<InterceptedChannel>;
type CandleDiffusionClient =
    pb::candle_diffusion_service_client::CandleDiffusionServiceClient<InterceptedChannel>;
type OnnxClient = pb::onnx_service_client::OnnxServiceClient<InterceptedChannel>;

fn ggml_llama_client(channel: Channel) -> (GgmlLlamaClient, String) {
    let interceptor = RequestIdInterceptor::new();
    let request_id = interceptor.id().to_owned();
    let client = pb::ggml_llama_service_client::GgmlLlamaServiceClient::with_interceptor(
        channel,
        interceptor,
    )
    .max_decoding_message_size(MAX_MESSAGE_BYTES)
    .max_encoding_message_size(MAX_MESSAGE_BYTES);
    (client, request_id)
}

fn ggml_whisper_client(channel: Channel) -> (GgmlWhisperClient, String) {
    let interceptor = RequestIdInterceptor::new();
    let request_id = interceptor.id().to_owned();
    let client = pb::ggml_whisper_service_client::GgmlWhisperServiceClient::with_interceptor(
        channel,
        interceptor,
    )
    .max_decoding_message_size(MAX_MESSAGE_BYTES)
    .max_encoding_message_size(MAX_MESSAGE_BYTES);
    (client, request_id)
}

fn ggml_diffusion_client(channel: Channel) -> (GgmlDiffusionClient, String) {
    let interceptor = RequestIdInterceptor::new();
    let request_id = interceptor.id().to_owned();
    let client = pb::ggml_diffusion_service_client::GgmlDiffusionServiceClient::with_interceptor(
        channel,
        interceptor,
    )
    .max_decoding_message_size(MAX_MESSAGE_BYTES)
    .max_encoding_message_size(MAX_MESSAGE_BYTES);
    (client, request_id)
}

fn candle_transformers_client(channel: Channel) -> (CandleTransformersClient, String) {
    let interceptor = RequestIdInterceptor::new();
    let request_id = interceptor.id().to_owned();
    let client =
        pb::candle_transformers_service_client::CandleTransformersServiceClient::with_interceptor(
            channel,
            interceptor,
        )
        .max_decoding_message_size(MAX_MESSAGE_BYTES)
        .max_encoding_message_size(MAX_MESSAGE_BYTES);
    (client, request_id)
}

fn candle_diffusion_client(channel: Channel) -> (CandleDiffusionClient, String) {
    let interceptor = RequestIdInterceptor::new();
    let request_id = interceptor.id().to_owned();
    let client =
        pb::candle_diffusion_service_client::CandleDiffusionServiceClient::with_interceptor(
            channel,
            interceptor,
        )
        .max_decoding_message_size(MAX_MESSAGE_BYTES)
        .max_encoding_message_size(MAX_MESSAGE_BYTES);
    (client, request_id)
}

fn onnx_client(channel: Channel) -> (OnnxClient, String) {
    let interceptor = RequestIdInterceptor::new();
    let request_id = interceptor.id().to_owned();
    let client = pb::onnx_service_client::OnnxServiceClient::with_interceptor(channel, interceptor)
        .max_decoding_message_size(MAX_MESSAGE_BYTES)
        .max_encoding_message_size(MAX_MESSAGE_BYTES);
    (client, request_id)
}

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

fn with_request_timeout<T>(message: T) -> tonic::Request<T> {
    let mut request = tonic::Request::new(message);
    request.set_timeout(RPC_REQUEST_TIMEOUT);
    request
}

async fn call_initial_response_with_retry<T, F, Fut>(
    rpc: &'static str,
    mut invoke: F,
) -> Result<tonic::Response<T>, tonic::Status>
where
    F: FnMut() -> (Fut, String),
    Fut: Future<Output = Result<tonic::Response<T>, tonic::Status>>,
{
    for attempt in 1..=UNARY_RPC_MAX_ATTEMPTS {
        let (future, request_id) = invoke();
        match future.await {
            Ok(response) => return Ok(response),
            Err(status) => {
                log_grpc_error(rpc, &request_id, &status);
                if is_transient_runtime_status(&status) && attempt < UNARY_RPC_MAX_ATTEMPTS {
                    warn!(
                        rpc,
                        request_id,
                        attempt,
                        max_attempts = UNARY_RPC_MAX_ATTEMPTS,
                        grpc.code = %status.code(),
                        grpc.message = %status.message(),
                        retry_delay_ms = UNARY_RPC_RETRY_DELAY.as_millis(),
                        "gRPC request failed with transient transport error before response; retrying"
                    );
                    tokio::time::sleep(UNARY_RPC_RETRY_DELAY).await;
                    continue;
                }
                return Err(status);
            }
        }
    }

    unreachable!("unary gRPC retry loop should always return")
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

pub async fn chat(
    channel: Channel,
    req: pb::GgmlLlamaChatRequest,
) -> anyhow::Result<pb::GgmlLlamaChatResponse> {
    debug!("sending gRPC ggml llama chat request");
    let response = call_initial_response_with_retry("chat", || {
        let (mut client, request_id) = ggml_llama_client(channel.clone());
        let request = with_request_timeout(req.clone());
        (async move { client.chat(request).await }, request_id)
    })
    .await?;
    Ok(response.into_inner())
}

pub async fn chat_stream(
    channel: Channel,
    req: pb::GgmlLlamaChatRequest,
) -> anyhow::Result<tonic::Streaming<pb::GgmlLlamaChatStreamChunk>> {
    debug!("sending gRPC ggml llama chat_stream request");
    let response = call_initial_response_with_retry("chat_stream", || {
        let (mut client, request_id) = ggml_llama_client(channel.clone());
        let request = with_request_timeout(req.clone());
        (async move { client.chat_stream(request).await }, request_id)
    })
    .await?;
    Ok(response.into_inner())
}

pub async fn candle_chat(
    channel: Channel,
    req: pb::CandleChatRequest,
) -> anyhow::Result<pb::CandleChatResponse> {
    debug!("sending gRPC candle llama chat request");
    let response = call_initial_response_with_retry("candle_chat", || {
        let (mut client, request_id) = candle_transformers_client(channel.clone());
        let request = with_request_timeout(req.clone());
        (async move { client.chat(request).await }, request_id)
    })
    .await?;
    Ok(response.into_inner())
}

pub async fn candle_chat_stream(
    channel: Channel,
    req: pb::CandleChatRequest,
) -> anyhow::Result<tonic::Streaming<pb::CandleChatStreamChunk>> {
    debug!("sending gRPC candle llama chat_stream request");
    let response = call_initial_response_with_retry("candle_chat_stream", || {
        let (mut client, request_id) = candle_transformers_client(channel.clone());
        let request = with_request_timeout(req.clone());
        (async move { client.chat_stream(request).await }, request_id)
    })
    .await?;
    Ok(response.into_inner())
}

pub async fn transcribe(
    channel: Channel,
    req: pb::GgmlWhisperTranscribeRequest,
) -> anyhow::Result<pb::GgmlWhisperTranscribeResponse> {
    let vad_enabled = req.vad.as_ref().and_then(|v| v.enabled).unwrap_or_default();
    let decode_configured = req.decode.is_some();
    debug!(
        audio_path = %req.path.as_deref().unwrap_or_default(),
        vad_enabled,
        decode_configured,
        "sending gRPC ggml whisper transcribe request"
    );
    let response = call_initial_response_with_retry("transcribe", || {
        let (mut client, request_id) = ggml_whisper_client(channel.clone());
        let request = with_request_timeout(req.clone());
        (async move { client.transcribe(request).await }, request_id)
    })
    .await
    .map_err(|status| grpc_status_to_anyhow("transcribe", "retry-exhausted", status))?;
    Ok(response.into_inner())
}

pub async fn candle_transcribe(
    channel: Channel,
    req: pb::CandleWhisperTranscribeRequest,
) -> anyhow::Result<pb::CandleWhisperTranscribeResponse> {
    debug!(
        audio_path = %req.path.as_deref().unwrap_or_default(),
        "sending gRPC candle whisper transcribe request"
    );
    let response = call_initial_response_with_retry("candle_transcribe", || {
        let (mut client, request_id) = candle_transformers_client(channel.clone());
        let request = with_request_timeout(req.clone());
        (async move { client.transcribe(request).await }, request_id)
    })
    .await
    .map_err(|status| grpc_status_to_anyhow("candle_transcribe", "retry-exhausted", status))?;
    Ok(response.into_inner())
}

pub async fn generate_image(
    channel: Channel,
    req: pb::GgmlDiffusionGenerateImageRequest,
) -> anyhow::Result<pb::GgmlDiffusionGenerateImageResponse> {
    debug!("sending gRPC ggml diffusion generate_image request");
    let response = call_initial_response_with_retry("generate_image", || {
        let (mut client, request_id) = ggml_diffusion_client(channel.clone());
        let request = with_request_timeout(req.clone());
        (async move { client.generate_image(request).await }, request_id)
    })
    .await
    .map_err(|status| grpc_status_to_anyhow("generate_image", "retry-exhausted", status))?;
    Ok(response.into_inner())
}

pub async fn candle_generate_image(
    channel: Channel,
    req: pb::CandleDiffusionGenerateImageRequest,
) -> anyhow::Result<pb::CandleDiffusionGenerateImageResponse> {
    debug!("sending gRPC candle diffusion generate_image request");
    let response = call_initial_response_with_retry("candle_generate_image", || {
        let (mut client, request_id) = candle_diffusion_client(channel.clone());
        let request = with_request_timeout(req.clone());
        (async move { client.generate_image(request).await }, request_id)
    })
    .await
    .map_err(|status| grpc_status_to_anyhow("candle_generate_image", "retry-exhausted", status))?;
    Ok(response.into_inner())
}

pub async fn generate_video(
    channel: Channel,
    req: pb::GgmlDiffusionGenerateVideoRequest,
) -> anyhow::Result<pb::GgmlDiffusionGenerateVideoResponse> {
    debug!("sending gRPC ggml diffusion generate_video request");
    let response = call_initial_response_with_retry("generate_video", || {
        let (mut client, request_id) = ggml_diffusion_client(channel.clone());
        let request = with_request_timeout(req.clone());
        (async move { client.generate_video(request).await }, request_id)
    })
    .await
    .map_err(|status| grpc_status_to_anyhow("generate_video", "retry-exhausted", status))?;
    Ok(response.into_inner())
}

pub async fn load_model(
    channel: Channel,
    req: ModelLoadRpcRequest,
) -> anyhow::Result<pb::ModelStatusResponse> {
    let backend_id = req.backend_id();
    let model_path = req.model_path().unwrap_or_default().to_owned();

    debug!(
        backend = %backend_id,
        model_path = %model_path,
        "sending gRPC load_model request"
    );

    for attempt in 1..=LOAD_MODEL_MAX_ATTEMPTS {
        let (response, request_id) = load_model_once(channel.clone(), req.clone()).await;

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

async fn load_model_once(
    channel: Channel,
    req: ModelLoadRpcRequest,
) -> (Result<tonic::Response<pb::ModelStatusResponse>, tonic::Status>, String) {
    match req {
        ModelLoadRpcRequest::GgmlLlama(req) => {
            let (mut client, request_id) = ggml_llama_client(channel);
            let result = client.load_model(with_request_timeout(req)).await.inspect_err(|status| {
                log_grpc_error("load_model", &request_id, status);
            });
            (result, request_id)
        }
        ModelLoadRpcRequest::GgmlWhisper(req) => {
            let (mut client, request_id) = ggml_whisper_client(channel);
            let result = client.load_model(with_request_timeout(req)).await.inspect_err(|status| {
                log_grpc_error("load_model", &request_id, status);
            });
            (result, request_id)
        }
        ModelLoadRpcRequest::GgmlDiffusion(req) => {
            let (mut client, request_id) = ggml_diffusion_client(channel);
            let result = client.load_model(with_request_timeout(req)).await.inspect_err(|status| {
                log_grpc_error("load_model", &request_id, status);
            });
            (result, request_id)
        }
        ModelLoadRpcRequest::CandleLlama(req) => {
            let (mut client, request_id) = candle_transformers_client(channel);
            let result =
                client.load_llama_model(with_request_timeout(req)).await.inspect_err(|status| {
                    log_grpc_error("load_llama_model", &request_id, status);
                });
            (result, request_id)
        }
        ModelLoadRpcRequest::CandleWhisper(req) => {
            let (mut client, request_id) = candle_transformers_client(channel);
            let result =
                client.load_whisper_model(with_request_timeout(req)).await.inspect_err(|status| {
                    log_grpc_error("load_whisper_model", &request_id, status);
                });
            (result, request_id)
        }
        ModelLoadRpcRequest::CandleDiffusion(req) => {
            let (mut client, request_id) = candle_diffusion_client(channel);
            let result = client.load_model(with_request_timeout(req)).await.inspect_err(|status| {
                log_grpc_error("load_model", &request_id, status);
            });
            (result, request_id)
        }
        ModelLoadRpcRequest::OnnxText(req) => {
            let (mut client, request_id) = onnx_client(channel);
            let result =
                client.load_text_model(with_request_timeout(req)).await.inspect_err(|status| {
                    log_grpc_error("load_text_model", &request_id, status);
                });
            (result, request_id)
        }
    }
}

pub async fn unload_model(
    channel: Channel,
    backend_id: RuntimeBackendId,
    req: pb::ModelUnloadRequest,
) -> anyhow::Result<pb::ModelStatusResponse> {
    debug!(backend = %backend_id, "sending gRPC unload_model request");

    for attempt in 1..=UNARY_RPC_MAX_ATTEMPTS {
        let (response, request_id) =
            unload_model_once(channel.clone(), backend_id, req.clone()).await?;
        match response {
            Ok(response) => return Ok(response.into_inner()),
            Err(status) => {
                if is_transient_runtime_status(&status) && attempt < UNARY_RPC_MAX_ATTEMPTS {
                    warn!(
                        backend = %backend_id,
                        request_id = %request_id,
                        attempt,
                        max_attempts = UNARY_RPC_MAX_ATTEMPTS,
                        grpc.code = %status.code(),
                        grpc.message = %status.message(),
                        retry_delay_ms = UNARY_RPC_RETRY_DELAY.as_millis(),
                        "gRPC unload_model failed with transient transport error before response; retrying"
                    );
                    tokio::time::sleep(UNARY_RPC_RETRY_DELAY).await;
                    continue;
                }

                return Err(anyhow::Error::from(status).context(format!(
                    "unload_model RPC failed for backend: {backend_id} (request_id={request_id}, attempt={attempt})"
                )));
            }
        }
    }

    unreachable!("unload_model retry loop should always return")
}

async fn unload_model_once(
    channel: Channel,
    backend_id: RuntimeBackendId,
    req: pb::ModelUnloadRequest,
) -> anyhow::Result<(Result<tonic::Response<pb::ModelStatusResponse>, tonic::Status>, String)> {
    let value = match backend_id {
        RuntimeBackendId::GgmlLlama => {
            let (mut client, request_id) = ggml_llama_client(channel);
            let result =
                client.unload_model(with_request_timeout(req)).await.inspect_err(|status| {
                    log_grpc_error("unload_model", &request_id, status);
                });
            (result, request_id)
        }
        RuntimeBackendId::GgmlWhisper => {
            let (mut client, request_id) = ggml_whisper_client(channel);
            let result =
                client.unload_model(with_request_timeout(req)).await.inspect_err(|status| {
                    log_grpc_error("unload_model", &request_id, status);
                });
            (result, request_id)
        }
        RuntimeBackendId::GgmlDiffusion => {
            let (mut client, request_id) = ggml_diffusion_client(channel);
            let result =
                client.unload_model(with_request_timeout(req)).await.inspect_err(|status| {
                    log_grpc_error("unload_model", &request_id, status);
                });
            (result, request_id)
        }
        RuntimeBackendId::CandleLlama => {
            let (mut client, request_id) = candle_transformers_client(channel);
            let result =
                client.unload_llama_model(with_request_timeout(req)).await.inspect_err(|status| {
                    log_grpc_error("unload_llama_model", &request_id, status);
                });
            (result, request_id)
        }
        RuntimeBackendId::CandleWhisper => {
            let (mut client, request_id) = candle_transformers_client(channel);
            let result = client.unload_whisper_model(with_request_timeout(req)).await.inspect_err(
                |status| {
                    log_grpc_error("unload_whisper_model", &request_id, status);
                },
            );
            (result, request_id)
        }
        RuntimeBackendId::CandleDiffusion => {
            let (mut client, request_id) = candle_diffusion_client(channel);
            let result =
                client.unload_model(with_request_timeout(req)).await.inspect_err(|status| {
                    log_grpc_error("unload_model", &request_id, status);
                });
            (result, request_id)
        }
        RuntimeBackendId::Onnx => {
            let (mut client, request_id) = onnx_client(channel);
            let result =
                client.unload_text_model(with_request_timeout(req)).await.inspect_err(|status| {
                    log_grpc_error("unload_text_model", &request_id, status);
                });
            (result, request_id)
        }
        other => {
            anyhow::bail!("backend {other} is not served by the gRPC runtime");
        }
    };

    Ok(value)
}
