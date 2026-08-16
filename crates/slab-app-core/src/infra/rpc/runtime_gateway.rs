use std::sync::Arc;

use async_trait::async_trait;
use futures::{StreamExt, stream::BoxStream};
use serde_json::{Value, json};
use slab_runtime_core::{RUNTIME_ERROR_CODE_METADATA, RUNTIME_ERROR_DETAIL_METADATA_BIN};
use slab_types::{RuntimeBackendId, RuntimeBackendLoadSpec};
use tonic::transport::Channel;

use crate::domain::ports::{
    RuntimeBackendStatus, RuntimeDiffusionImageRequest, RuntimeDiffusionImageResult,
    RuntimeDiffusionVideoRequest, RuntimeDiffusionVideoResult, RuntimeInferenceGateway,
    RuntimeQuantizeRequest, RuntimeQuantizeResult, RuntimeTextGenerationChunk,
    RuntimeTextGenerationRequest, RuntimeTextGenerationResponse, RuntimeTranscriptionDecodeOptions,
    RuntimeTranscriptionRequest, RuntimeTranscriptionResult, RuntimeTranscriptionVadOptions,
    RuntimeTranscriptionVadParams,
};
use crate::error::AppCoreError;
use crate::error::AppCoreErrorData;

use super::{client, codec, gateway::GrpcGateway, pb, runtime_protocol};

#[derive(Clone)]
pub struct GrpcRuntimeInferenceGateway {
    grpc: Arc<GrpcGateway>,
}

impl GrpcRuntimeInferenceGateway {
    pub fn new(grpc: Arc<GrpcGateway>) -> Self {
        Self { grpc }
    }

    fn channel(&self, backend_id: RuntimeBackendId) -> Result<Channel, AppCoreError> {
        self.grpc.backend_channel(backend_id).ok_or_else(|| {
            AppCoreError::BackendNotReady(format!(
                "{} gRPC endpoint is not configured",
                backend_id.canonical_id()
            ))
        })
    }
}

impl std::fmt::Debug for GrpcRuntimeInferenceGateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GrpcRuntimeInferenceGateway").finish_non_exhaustive()
    }
}

#[async_trait]
impl RuntimeInferenceGateway for GrpcRuntimeInferenceGateway {
    fn backend_available(&self, backend_id: RuntimeBackendId) -> bool {
        self.grpc.backend_channel(backend_id).is_some()
    }

    async fn chat(
        &self,
        request: RuntimeTextGenerationRequest,
    ) -> Result<RuntimeTextGenerationResponse, AppCoreError> {
        let backend_id = request.backend_id.unwrap_or(RuntimeBackendId::GgmlLlama);
        match backend_id {
            RuntimeBackendId::GgmlLlama => {
                let channel = self.channel(backend_id)?;
                let grpc_request = runtime_protocol::encode_chat_request(&request);
                let response = retry_on_session_busy("chat", || {
                    client::chat(channel.clone(), grpc_request.clone())
                })
                .await?;
                Ok(runtime_protocol::decode_chat_response(&response))
            }
            RuntimeBackendId::CandleLlama => {
                let channel = self.channel(backend_id)?;
                let grpc_request = runtime_protocol::encode_candle_chat_request(&request);
                let response = client::candle_chat(channel, grpc_request)
                    .await
                    .map_err(map_runtime_error("candle chat"))?;
                Ok(runtime_protocol::decode_candle_chat_response(&response))
            }
            other => Err(unsupported_inference_backend("chat", other)),
        }
    }

    async fn chat_stream(
        &self,
        request: RuntimeTextGenerationRequest,
    ) -> Result<BoxStream<'static, Result<RuntimeTextGenerationChunk, AppCoreError>>, AppCoreError>
    {
        let backend_id = request.backend_id.unwrap_or(RuntimeBackendId::GgmlLlama);
        match backend_id {
            RuntimeBackendId::GgmlLlama => {
                let channel = self.channel(backend_id)?;
                let grpc_request = runtime_protocol::encode_chat_request(&request);
                let stream = retry_on_session_busy("chat stream", || {
                    client::chat_stream(channel.clone(), grpc_request.clone())
                })
                .await?;
                Ok(stream
                    .map(|chunk| {
                        chunk
                            .map(|chunk| runtime_protocol::decode_chat_stream_chunk(&chunk))
                            .map_err(map_runtime_status("chat stream"))
                    })
                    .boxed())
            }
            RuntimeBackendId::CandleLlama => {
                let channel = self.channel(backend_id)?;
                let grpc_request = runtime_protocol::encode_candle_chat_request(&request);
                let stream = client::candle_chat_stream(channel, grpc_request)
                    .await
                    .map_err(map_runtime_error("candle chat stream"))?;
                Ok(stream
                    .map(|chunk| {
                        chunk
                            .map(|chunk| runtime_protocol::decode_candle_chat_stream_chunk(&chunk))
                            .map_err(map_runtime_status("candle chat stream"))
                    })
                    .boxed())
            }
            other => Err(unsupported_inference_backend("chat stream", other)),
        }
    }

    async fn transcribe(
        &self,
        request: RuntimeTranscriptionRequest,
    ) -> Result<RuntimeTranscriptionResult, AppCoreError> {
        let backend_id = request.backend_id.unwrap_or(RuntimeBackendId::GgmlWhisper);
        match backend_id {
            RuntimeBackendId::GgmlWhisper => {
                let channel = self.channel(backend_id)?;
                let response =
                    client::transcribe(channel, pb_whisper_request_from_runtime(request))
                        .await
                        .map_err(map_runtime_error("transcribe"))?;
                Ok(runtime_protocol::decode_whisper_transcription_response(&response))
            }
            RuntimeBackendId::GgmlParakeet => {
                let channel = self.channel(backend_id)?;
                let response =
                    client::parakeet_transcribe(channel, pb_parakeet_request_from_runtime(request))
                        .await
                        .map_err(map_runtime_error("transcribe"))?;
                Ok(runtime_protocol::decode_parakeet_transcription_response(&response))
            }
            RuntimeBackendId::CandleWhisper => {
                let channel = self.channel(backend_id)?;
                let response = client::candle_transcribe(
                    channel,
                    pb_candle_whisper_request_from_runtime(request),
                )
                .await
                .map_err(map_runtime_error("candle transcribe"))?;
                Ok(runtime_protocol::decode_candle_whisper_transcription_response(&response))
            }
            other => Err(unsupported_inference_backend("transcribe", other)),
        }
    }

    async fn generate_image(
        &self,
        request: RuntimeDiffusionImageRequest,
    ) -> Result<RuntimeDiffusionImageResult, AppCoreError> {
        let backend_id = request.backend_id.unwrap_or(RuntimeBackendId::GgmlDiffusion);
        match backend_id {
            RuntimeBackendId::GgmlDiffusion => {
                let channel = self.channel(backend_id)?;
                let grpc_request = runtime_protocol::encode_diffusion_image_request(&request);
                let response = client::generate_image(channel, grpc_request)
                    .await
                    .map_err(map_runtime_error("generate image"))?;
                runtime_protocol::decode_diffusion_image_response(&response).map_err(|error| {
                    AppCoreError::Internal(format!(
                        "invalid diffusion image response payload: {error}"
                    ))
                })
            }
            RuntimeBackendId::CandleDiffusion => {
                let channel = self.channel(backend_id)?;
                let grpc_request =
                    runtime_protocol::encode_candle_diffusion_image_request(&request);
                let response = client::candle_generate_image(channel, grpc_request)
                    .await
                    .map_err(map_runtime_error("candle generate image"))?;
                runtime_protocol::decode_candle_diffusion_image_response(&response).map_err(
                    |error| {
                        AppCoreError::Internal(format!(
                            "invalid candle diffusion image response payload: {error}"
                        ))
                    },
                )
            }
            other => Err(unsupported_inference_backend("generate image", other)),
        }
    }

    async fn generate_video(
        &self,
        request: RuntimeDiffusionVideoRequest,
    ) -> Result<RuntimeDiffusionVideoResult, AppCoreError> {
        let channel = self.channel(RuntimeBackendId::GgmlDiffusion)?;
        let grpc_request = runtime_protocol::encode_diffusion_video_request(&request);
        let response = client::generate_video(channel, grpc_request)
            .await
            .map_err(map_runtime_error("generate video"))?;
        runtime_protocol::decode_diffusion_video_response(&response).map_err(|error| {
            AppCoreError::Internal(format!("invalid diffusion video response payload: {error}"))
        })
    }

    async fn load_model(
        &self,
        spec: &RuntimeBackendLoadSpec,
    ) -> Result<RuntimeBackendStatus, AppCoreError> {
        let channel = self.channel(spec.backend())?;
        let request = codec::encode_model_load_request(spec);
        let response = client::load_model(channel, request).await.map_err(map_model_load_error)?;
        runtime_status_from_pb(response)
    }

    async fn unload_model(
        &self,
        backend_id: RuntimeBackendId,
    ) -> Result<RuntimeBackendStatus, AppCoreError> {
        let channel = self.channel(backend_id)?;
        let response = client::unload_model(channel, backend_id, pb::ModelUnloadRequest::default())
            .await
            .map_err(map_runtime_error("unload model"))?;
        runtime_status_from_pb(response)
    }

    async fn quantize_model(
        &self,
        request: RuntimeQuantizeRequest,
    ) -> Result<RuntimeQuantizeResult, AppCoreError> {
        let backend_id = RuntimeBackendId::GgmlLlama;
        let channel = self.channel(backend_id)?;
        let grpc_request = pb::GgmlLlamaQuantizeRequest {
            input_path: Some(request.input_path),
            output_path: Some(request.output_path),
            ftype: Some(request.ftype),
            nthread: request.nthread,
            allow_requantize: request.allow_requantize,
            quantize_output_tensor: request.quantize_output_tensor,
            only_copy: request.only_copy,
            pure: request.pure,
            keep_split: request.keep_split,
            dry_run: request.dry_run,
        };
        let response = client::quantize_model(channel, grpc_request)
            .await
            .map_err(map_runtime_error("quantize model"))?;
        Ok(RuntimeQuantizeResult {
            layers_processed: response.layers_processed.unwrap_or(0),
            output_path: response.output_path.unwrap_or_default(),
        })
    }
}

fn map_runtime_error(action: &'static str) -> impl Fn(anyhow::Error) -> AppCoreError {
    move |error| {
        if let Some(error) = structured_runtime_failure(action, &error) {
            return error;
        }
        if let Some(detail) = client::transient_runtime_detail(&error) {
            return AppCoreError::BackendNotReady(detail);
        }
        if let Some(detail) = session_busy_detail(&error) {
            return AppCoreError::Conflict(detail);
        }
        if is_memory_pressure_error(&error) {
            return runtime_failure(
                "runtime_memory_pressure",
                format!("runtime reported memory pressure during {action}: {error:#}"),
                json!({
                    "action": action,
                    "message": error.to_string(),
                }),
            );
        }
        AppCoreError::Internal(format!("grpc {action} failed: {error:#}"))
    }
}

/// Run `op`, retrying on transient "session key busy" runtime errors.
///
/// The ggml.llama backend keys kv-cache sessions per agent and reports the key
/// as busy/already-active if a second inference for the same key arrives while
/// the previous one is still being torn down. This surfaces as rapid
/// back-to-back inference after an auto-allowed (non-approval-gating) tool such
/// as `plan` — unlike shell, whose approval gate inserts a delay that masks the
/// race. The busy state is transient (the prior inference just finished), so a
/// short bounded exponential backoff retries through it instead of failing the
/// turn. Non-busy errors are surfaced immediately via [`map_runtime_error`].
async fn retry_on_session_busy<F, Fut, T>(action: &'static str, op: F) -> Result<T, AppCoreError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, anyhow::Error>>,
{
    const MAX_ATTEMPTS: usize = 5;
    let mut backoff = std::time::Duration::from_millis(50);
    let mut attempt = 0;
    loop {
        attempt += 1;
        match op().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                if session_busy_detail(&error).is_some() && attempt < MAX_ATTEMPTS {
                    tokio::time::sleep(backoff).await;
                    backoff *= 2;
                    continue;
                }
                return Err(map_runtime_error(action)(error));
            }
        }
    }
}

fn map_model_load_error(error: anyhow::Error) -> AppCoreError {
    if let Some(error) = structured_runtime_failure("load model", &error) {
        return error;
    }
    if let Some(detail) = client::transient_runtime_detail(&error) {
        return AppCoreError::BackendNotReady(detail);
    }
    if is_memory_pressure_error(&error) {
        return runtime_failure(
            "runtime_memory_pressure",
            format!("runtime reported memory pressure during model load: {error:#}"),
            json!({
                "action": "load model",
                "message": error.to_string(),
            }),
        );
    }
    AppCoreError::Internal(format!("grpc load model failed: {error:#}"))
}

fn structured_runtime_failure(action: &'static str, error: &anyhow::Error) -> Option<AppCoreError> {
    let status = error.chain().find_map(|cause| cause.downcast_ref::<tonic::Status>())?;
    let runtime_code = status
        .metadata()
        .get(RUNTIME_ERROR_CODE_METADATA)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)?;
    let detail = runtime_detail_from_status(status).unwrap_or_else(|| {
        json!({
            "action": action,
            "grpc_code": format!("{:?}", status.code()),
            "message": status.message(),
        })
    });
    let message = if status.message().trim().is_empty() {
        format!("runtime {action} failed with {runtime_code}")
    } else {
        status.message().trim().to_owned()
    };
    Some(runtime_failure(runtime_code, message, detail))
}

fn runtime_detail_from_status(status: &tonic::Status) -> Option<Value> {
    let value = status.metadata().get_bin(RUNTIME_ERROR_DETAIL_METADATA_BIN)?;
    let bytes = value.to_bytes().ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn runtime_failure(
    runtime_code: impl Into<String>,
    message: impl Into<String>,
    detail: Value,
) -> AppCoreError {
    AppCoreError::RuntimeFailure {
        message: message.into(),
        data: Box::new(AppCoreErrorData::runtime_failure(runtime_code, detail)),
    }
}

fn session_busy_detail(error: &anyhow::Error) -> Option<String> {
    let status = error.chain().find_map(|cause| cause.downcast_ref::<tonic::Status>())?;
    if status.code() != tonic::Code::ResourceExhausted {
        return None;
    }

    let message = status.message().trim();
    let lower = message.to_ascii_lowercase();
    let session_busy = lower.contains("session key")
        && (lower.contains("busy") || lower.contains("already active"));
    session_busy.then(|| {
        if message.is_empty() {
            "runtime session key is busy".to_owned()
        } else {
            message.to_owned()
        }
    })
}

fn is_memory_pressure_error(error: &anyhow::Error) -> bool {
    let Some(status) = error.chain().find_map(|cause| cause.downcast_ref::<tonic::Status>()) else {
        return false;
    };
    slab_gpu_memory_scheduler::is_oom_message(status.message())
        && matches!(
            status.code(),
            tonic::Code::ResourceExhausted | tonic::Code::Internal | tonic::Code::Unknown
        )
}

fn map_runtime_status(action: &'static str) -> impl Fn(tonic::Status) -> AppCoreError {
    move |status| {
        let error = anyhow::Error::new(status);
        map_runtime_error(action)(error)
    }
}

fn unsupported_inference_backend(action: &str, backend_id: RuntimeBackendId) -> AppCoreError {
    AppCoreError::BadRequest(format!(
        "backend '{}' does not support runtime {action}",
        backend_id.canonical_id()
    ))
}

pub(crate) fn runtime_status_from_pb(
    response: pb::ModelStatusResponse,
) -> Result<RuntimeBackendStatus, AppCoreError> {
    runtime_protocol::decode_model_status_response(&response).map_err(|error| {
        AppCoreError::Internal(format!("invalid model status response from runtime: {error}"))
    })
}

fn pb_whisper_request_from_runtime(
    request: RuntimeTranscriptionRequest,
) -> pb::GgmlWhisperTranscribeRequest {
    pb::GgmlWhisperTranscribeRequest {
        path: Some(request.path),
        language: request.language,
        prompt: request.prompt,
        detect_language: request.detect_language,
        vad: request.vad.map(pb_whisper_vad_options_from_runtime),
        decode: request.decode.map(pb_whisper_decode_options_from_runtime),
    }
}

fn pb_candle_whisper_request_from_runtime(
    request: RuntimeTranscriptionRequest,
) -> pb::CandleWhisperTranscribeRequest {
    pb::CandleWhisperTranscribeRequest { path: Some(request.path) }
}

fn pb_parakeet_request_from_runtime(
    request: RuntimeTranscriptionRequest,
) -> pb::GgmlParakeetTranscribeRequest {
    pb::GgmlParakeetTranscribeRequest {
        path: Some(request.path),
        decode: request.decode.map(pb_parakeet_decode_options_from_runtime),
    }
}

fn pb_parakeet_decode_options_from_runtime(
    value: RuntimeTranscriptionDecodeOptions,
) -> pb::GgmlParakeetDecodeOptions {
    pb::GgmlParakeetDecodeOptions {
        offset_ms: value.offset_ms,
        duration_ms: value.duration_ms,
        no_context: value.no_context,
        audio_ctx: None,
        n_threads: None,
    }
}

fn pb_whisper_vad_options_from_runtime(
    value: RuntimeTranscriptionVadOptions,
) -> pb::GgmlWhisperVadOptions {
    pb::GgmlWhisperVadOptions {
        enabled: Some(value.enabled),
        model_path: value.model_path,
        params: value.params.map(pb_whisper_vad_params_from_runtime),
    }
}

fn pb_whisper_vad_params_from_runtime(
    value: RuntimeTranscriptionVadParams,
) -> pb::GgmlWhisperVadParams {
    pb::GgmlWhisperVadParams {
        threshold: value.threshold,
        min_speech_duration_ms: value.min_speech_duration_ms,
        min_silence_duration_ms: value.min_silence_duration_ms,
        max_speech_duration_s: value.max_speech_duration_s,
        speech_pad_ms: value.speech_pad_ms,
        samples_overlap: value.samples_overlap,
    }
}

fn pb_whisper_decode_options_from_runtime(
    value: RuntimeTranscriptionDecodeOptions,
) -> pb::GgmlWhisperDecodeOptions {
    pb::GgmlWhisperDecodeOptions {
        offset_ms: value.offset_ms,
        duration_ms: value.duration_ms,
        no_context: value.no_context,
        no_timestamps: value.no_timestamps,
        token_timestamps: value.token_timestamps,
        split_on_word: value.split_on_word,
        suppress_nst: value.suppress_nst,
        word_thold: value.word_thold,
        max_len: value.max_len,
        max_tokens: value.max_tokens,
        temperature: value.temperature,
        temperature_inc: value.temperature_inc,
        entropy_thold: value.entropy_thold,
        logprob_thold: value.logprob_thold,
        no_speech_thold: value.no_speech_thold,
        tdrz_enable: value.tdrz_enable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_runtime_error_reports_inference_memory_pressure() {
        let status = tonic::Status::new(
            tonic::Code::ResourceExhausted,
            "CUDA out of memory while allocating KV cache",
        );
        let error = map_runtime_error("chat")(anyhow::Error::new(status));

        assert!(
            matches!(&error, AppCoreError::RuntimeFailure { message, data }
                if message.contains("chat")
                    && data.runtime_code() == Some("runtime_memory_pressure")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn map_runtime_error_preserves_runtime_metadata() {
        let mut status = tonic::Status::new(tonic::Code::ResourceExhausted, "backend busy");
        status.metadata_mut().insert(
            RUNTIME_ERROR_CODE_METADATA,
            tonic::metadata::MetadataValue::try_from("runtime_backend_busy").unwrap(),
        );
        status.metadata_mut().insert_bin(
            RUNTIME_ERROR_DETAIL_METADATA_BIN,
            tonic::metadata::MetadataValue::from_bytes(br#"{"backend_id":"ggml.llama"}"#),
        );

        let error = map_runtime_error("chat")(anyhow::Error::new(status));

        let AppCoreError::RuntimeFailure { message, data } = error else {
            panic!("expected RuntimeFailure");
        };
        assert_eq!(message, "backend busy");
        assert_eq!(data.runtime_code(), Some("runtime_backend_busy"));
    }

    #[test]
    fn map_runtime_error_reports_session_busy_as_conflict() {
        let status = tonic::Status::new(
            tonic::Code::ResourceExhausted,
            "backend busy: ggml.llama session key 'chat-1'",
        );
        let error = map_runtime_error("chat")(anyhow::Error::new(status));

        assert!(
            matches!(&error, AppCoreError::Conflict(message) if message.contains("session key")),
            "unexpected error: {error}"
        );
    }

    use std::sync::atomic::{AtomicUsize, Ordering};

    fn session_busy_status() -> anyhow::Error {
        anyhow::Error::new(tonic::Status::new(
            tonic::Code::ResourceExhausted,
            "backend busy: ggml.llama session key 'agent:test' is busy",
        ))
    }

    #[tokio::test]
    async fn retry_on_session_busy_succeeds_after_transient_failures() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_clone = Arc::clone(&attempts);
        let result: Result<u32, AppCoreError> = retry_on_session_busy("chat", move || {
            let attempts = Arc::clone(&attempts_clone);
            async move {
                let n = attempts.fetch_add(1, Ordering::SeqCst);
                if n < 2 { Err(session_busy_status()) } else { Ok(7u32) }
            }
        })
        .await;
        assert_eq!(result.unwrap(), 7);
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn retry_on_session_busy_surfaces_conflict_after_max_attempts() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_clone = Arc::clone(&attempts);
        let result: Result<u32, AppCoreError> = retry_on_session_busy("chat", move || {
            let attempts = Arc::clone(&attempts_clone);
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err(session_busy_status())
            }
        })
        .await;
        assert!(matches!(result, Err(AppCoreError::Conflict(_))));
        assert_eq!(attempts.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn retry_on_session_busy_does_not_retry_non_busy_errors() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_clone = Arc::clone(&attempts);
        let result: Result<u32, AppCoreError> = retry_on_session_busy("chat", move || {
            let attempts = Arc::clone(&attempts_clone);
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err(anyhow::Error::new(tonic::Status::internal("unrelated failure")))
            }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
}
