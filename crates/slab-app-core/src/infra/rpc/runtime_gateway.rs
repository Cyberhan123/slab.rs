use std::sync::Arc;

use async_trait::async_trait;
use futures::{StreamExt, stream::BoxStream};
use slab_types::{RuntimeBackendId, RuntimeBackendLoadSpec};
use tonic::transport::Channel;

use crate::domain::ports::{
    RuntimeBackendStatus, RuntimeDiffusionImageRequest, RuntimeDiffusionImageResult,
    RuntimeDiffusionVideoRequest, RuntimeDiffusionVideoResult, RuntimeInferenceGateway,
    RuntimeTextGenerationChunk, RuntimeTextGenerationRequest, RuntimeTextGenerationResponse,
    RuntimeTranscriptionDecodeOptions, RuntimeTranscriptionRequest, RuntimeTranscriptionResult,
    RuntimeTranscriptionVadOptions, RuntimeTranscriptionVadParams,
};
use crate::error::AppCoreError;

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
        let channel = self.channel(RuntimeBackendId::GgmlLlama)?;
        let grpc_request = runtime_protocol::encode_chat_request(&request);
        let response =
            client::chat(channel, grpc_request).await.map_err(map_runtime_error("chat"))?;
        Ok(runtime_protocol::decode_chat_response(&response))
    }

    async fn chat_stream(
        &self,
        request: RuntimeTextGenerationRequest,
    ) -> Result<BoxStream<'static, Result<RuntimeTextGenerationChunk, AppCoreError>>, AppCoreError>
    {
        let channel = self.channel(RuntimeBackendId::GgmlLlama)?;
        let grpc_request = runtime_protocol::encode_chat_request(&request);
        let stream = client::chat_stream(channel, grpc_request)
            .await
            .map_err(map_runtime_error("chat stream"))?;
        Ok(stream
            .map(|chunk| {
                chunk
                    .map(|chunk| runtime_protocol::decode_chat_stream_chunk(&chunk))
                    .map_err(map_runtime_status("chat stream"))
            })
            .boxed())
    }

    async fn transcribe(
        &self,
        request: RuntimeTranscriptionRequest,
    ) -> Result<RuntimeTranscriptionResult, AppCoreError> {
        let channel = self.channel(RuntimeBackendId::GgmlWhisper)?;
        let response = client::transcribe(channel, pb_whisper_request_from_runtime(request))
            .await
            .map_err(map_runtime_error("transcribe"))?;
        Ok(runtime_protocol::decode_whisper_transcription_response(&response))
    }

    async fn generate_image(
        &self,
        request: RuntimeDiffusionImageRequest,
    ) -> Result<RuntimeDiffusionImageResult, AppCoreError> {
        let channel = self.channel(RuntimeBackendId::GgmlDiffusion)?;
        let grpc_request = runtime_protocol::encode_diffusion_image_request(&request);
        let response = client::generate_image(channel, grpc_request)
            .await
            .map_err(map_runtime_error("generate image"))?;
        runtime_protocol::decode_diffusion_image_response(&response).map_err(|error| {
            AppCoreError::Internal(format!("invalid diffusion image response payload: {error}"))
        })
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
}

fn map_runtime_error(action: &'static str) -> impl Fn(anyhow::Error) -> AppCoreError {
    move |error| {
        if let Some(detail) = client::transient_runtime_detail(&error) {
            return AppCoreError::BackendNotReady(detail);
        }
        AppCoreError::Internal(format!("grpc {action} failed: {error:#}"))
    }
}

fn map_model_load_error(error: anyhow::Error) -> AppCoreError {
    if let Some(detail) = client::transient_runtime_detail(&error) {
        return AppCoreError::BackendNotReady(detail);
    }
    if is_memory_pressure_error(&error) {
        return AppCoreError::RuntimeMemoryPressure(format!(
            "runtime reported memory pressure during model load: {error:#}"
        ));
    }
    AppCoreError::Internal(format!("grpc load model failed: {error:#}"))
}

fn is_memory_pressure_error(error: &anyhow::Error) -> bool {
    let Some(status) = error.chain().find_map(|cause| cause.downcast_ref::<tonic::Status>()) else {
        return false;
    };
    let message = status.message().trim().to_ascii_lowercase();
    let mentions_memory = [
        "out of memory",
        "not enough memory",
        "insufficient memory",
        "memory allocation",
        "memory",
        "oom",
        "vram",
        "cudaerrormemoryallocation",
    ]
    .iter()
    .any(|needle| message.contains(needle));

    mentions_memory
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
