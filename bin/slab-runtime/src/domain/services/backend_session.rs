use std::convert::Infallible;
use std::sync::Arc;

use futures::stream::BoxStream;
use slab_diffusion::{Image as DiffusionImage, ImgParams as DiffusionImgParams};
use slab_runtime_core::backend::{BackendOp, StreamChunk};
use slab_runtime_core::scheduler::{CpuStage, PipelineBuilder};
use slab_runtime_core::{CoreError, Payload};
use slab_types::{
    AudioTranscriptionRequest, AudioTranscriptionResponse, Capability, ImageEmbeddingRequest,
    ImageEmbeddingResponse, ImageGenerationRequest, ImageGenerationResponse, ModelSpec,
    TextGenerationChunk, TextGenerationRequest, TextGenerationResponse,
};
use tokio::sync::Mutex;

use super::codec::{
    decode_audio_transcription_response, decode_image_embedding_response,
    decode_image_generation_response, decode_text_generation_chunk,
    decode_text_generation_response, encode_audio_transcription_options,
    encode_image_embedding_request, encode_image_generation_request, encode_load_payload,
    encode_text_generation_request, image_embedding_input_name,
    image_embedding_output_name,
};
use super::execution_hub::ExecutionHub;
use crate::domain::models::{InvocationPlan, ResolvedBackend, TaskCodec, TaskHandle};

#[derive(Clone, Debug)]
pub struct BackendSession {
    execution: ExecutionHub,
    spec: Arc<ModelSpec>,
    bound_backend_target: Arc<str>,
    deployment: Arc<Mutex<Option<LoadedDeployment>>>,
}

#[derive(Clone, Debug)]
struct LoadedDeployment {
    resolved: ResolvedBackend,
}

impl BackendSession {
    pub(crate) fn new_for_backend(
        execution: ExecutionHub,
        spec: ModelSpec,
        backend_target: impl Into<String>,
    ) -> Result<Self, CoreError> {
        Ok(Self {
            execution,
            spec: Arc::new(spec),
            bound_backend_target: Arc::<str>::from(backend_target.into()),
            deployment: Arc::new(Mutex::new(None)),
        })
    }

    pub fn model(&self) -> &ModelSpec {
        self.spec.as_ref()
    }

    pub fn capability(&self) -> Capability {
        self.spec.capability
    }

    pub async fn load(&self) -> Result<(), CoreError> {
        let _ = self.ensure_loaded_for(self.spec.capability, false).await?;
        Ok(())
    }

    pub async fn unload(&self) -> Result<(), CoreError> {
        let resolved = {
            let guard = self.deployment.lock().await;
            guard.as_ref().map(|deployment| deployment.resolved.clone())
        };

        if let Some(resolved) = resolved {
            self.execution.orchestrator().unload_model_backend(&resolved.backend_id).await?;
            let mut guard = self.deployment.lock().await;
            *guard = None;
        }

        Ok(())
    }

    pub async fn run_text_generation(
        &self,
        mut request: TextGenerationRequest,
    ) -> Result<TextGenerationResponse, CoreError> {
        request.stream = false;
        self.submit_text_generation(request).await?.result().await
    }

    pub async fn stream_text_generation(
        &self,
        mut request: TextGenerationRequest,
    ) -> Result<BoxStream<'static, Result<TextGenerationChunk, CoreError>>, CoreError> {
        request.stream = true;
        self.submit_text_generation(request).await?.take_stream().await
    }

    pub async fn submit_text_generation(
        &self,
        request: TextGenerationRequest,
    ) -> Result<TaskHandle<TextGenerationResponse, TextGenerationChunk>, CoreError> {
        self.require_capability(Capability::TextGeneration)?;
        let deployment = self.ensure_loaded_for(Capability::TextGeneration, request.stream).await?;
        let resolved = deployment.resolved.clone();
        let (input, op_options) = encode_text_generation_request(&request, &resolved)?;
        let plan = InvocationPlan::new(
            resolved,
            Capability::TextGeneration,
            request.stream,
            input,
            Vec::new(),
            op_options,
        )?;
        submit_plan(&self.execution, plan, TextGenerationTaskCodec).await
    }

    pub async fn run_audio_transcription(
        &self,
        request: AudioTranscriptionRequest,
    ) -> Result<AudioTranscriptionResponse, CoreError> {
        self.submit_audio_transcription(request).await?.result().await
    }

    pub async fn submit_audio_transcription(
        &self,
        request: AudioTranscriptionRequest,
    ) -> Result<TaskHandle<AudioTranscriptionResponse, Infallible>, CoreError> {
        self.require_capability(Capability::AudioTranscription)?;
        let deployment = self.ensure_loaded_for(Capability::AudioTranscription, false).await?;
        let resolved = deployment.resolved.clone();
        let input = request.pcm_samples.clone().map(Payload::F32).unwrap_or(Payload::None);
        let stages = if request.pcm_samples.is_some() {
            Vec::new()
        } else {
            let path = request.audio_path.clone();
            vec![CpuStage::new("audio.decode.wav", move |_| decode_wav_payload(&path))]
        };
        let op_options = encode_audio_transcription_options(&request, &resolved)?;
        let plan = InvocationPlan::new(
            resolved,
            Capability::AudioTranscription,
            false,
            input,
            stages,
            op_options,
        )?;
        submit_plan(
            &self.execution,
            plan,
            AudioTranscriptionTaskCodec { fallback_language: request.language.clone() },
        )
        .await
    }

    pub async fn run_image_generation(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse, CoreError> {
        self.submit_image_generation(request).await?.result().await
    }

    pub async fn run_inference_image(
        &self,
        params: DiffusionImgParams,
    ) -> Result<Vec<DiffusionImage>, CoreError> {
        self.submit_inference_image(params).await?.result().await
    }

    pub async fn submit_image_generation(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<TaskHandle<ImageGenerationResponse, Infallible>, CoreError> {
        self.require_capability(Capability::ImageGeneration)?;
        let deployment = self.ensure_loaded_for(Capability::ImageGeneration, false).await?;
        let resolved = deployment.resolved.clone();
        let (input, op_options) = encode_image_generation_request(&request, &resolved)?;
        let plan = InvocationPlan::new(
            resolved,
            Capability::ImageGeneration,
            false,
            input,
            Vec::new(),
            op_options,
        )?;
        submit_plan(&self.execution, plan, ImageGenerationTaskCodec).await
    }

    pub async fn submit_inference_image(
        &self,
        params: DiffusionImgParams,
    ) -> Result<TaskHandle<Vec<DiffusionImage>, Infallible>, CoreError> {
        self.require_capability(Capability::ImageGeneration)?;
        let deployment = self.ensure_loaded_for(Capability::ImageGeneration, false).await?;
        let resolved = deployment.resolved.clone();
        let plan = InvocationPlan::new(
            resolved,
            Capability::ImageGeneration,
            false,
            Payload::typed(params),
            Vec::new(),
            Payload::None,
        )?;
        submit_plan(&self.execution, plan, InferenceImageTaskCodec).await
    }

    pub async fn run_image_embedding(
        &self,
        request: ImageEmbeddingRequest,
    ) -> Result<ImageEmbeddingResponse, CoreError> {
        self.submit_image_embedding(request).await?.result().await
    }

    pub async fn submit_image_embedding(
        &self,
        request: ImageEmbeddingRequest,
    ) -> Result<TaskHandle<ImageEmbeddingResponse, Infallible>, CoreError> {
        self.require_capability(Capability::ImageEmbedding)?;
        let deployment = self.ensure_loaded_for(Capability::ImageEmbedding, false).await?;
        let resolved = deployment.resolved.clone();
        let input_name = image_embedding_input_name(self.spec.as_ref());
        let output_name = image_embedding_output_name(self.spec.as_ref());
        let (input, op_options) = encode_image_embedding_request(&request, &input_name)?;
        let plan = InvocationPlan::new(
            resolved,
            Capability::ImageEmbedding,
            false,
            input,
            Vec::new(),
            op_options,
        )?;
        submit_plan(&self.execution, plan, ImageEmbeddingTaskCodec { output_name }).await
    }

    async fn ensure_loaded_for(
        &self,
        capability: Capability,
        streaming: bool,
    ) -> Result<LoadedDeployment, CoreError> {
        {
            let guard = self.deployment.lock().await;
            if let Some(existing) = guard.as_ref() {
                if streaming && !existing.resolved.supports_streaming {
                    return Err(CoreError::UnsupportedOperation {
                        backend: existing.resolved.driver_id.clone(),
                        op: "stream".to_owned(),
                    });
                }
                if existing.resolved.capability != capability {
                    return Err(CoreError::UnsupportedCapability {
                        family: format!("{:?}", existing.resolved.family),
                        capability: format!("{:?}", capability),
                    });
                }
                return Ok(existing.clone());
            }
        }

        let resolved = self.execution.catalog().bind_for_target(
            self.spec.as_ref(),
            self.bound_backend_target.as_ref(),
            capability,
            streaming,
        )?;
        let payload = encode_load_payload(self.spec.as_ref(), &resolved)?;
        self.execution.orchestrator().load_model_backend(&resolved.backend_id, payload).await?;

        let deployment = LoadedDeployment { resolved };

        let mut guard = self.deployment.lock().await;
        *guard = Some(deployment.clone());
        Ok(deployment)
    }

    fn require_capability(&self, capability: Capability) -> Result<(), CoreError> {
        if self.spec.capability == capability {
            Ok(())
        } else {
            Err(CoreError::UnsupportedCapability {
                family: format!("{:?}", self.spec.family),
                capability: format!("{:?}", capability),
            })
        }
    }
}

struct TextGenerationTaskCodec;

impl TaskCodec<TextGenerationResponse, TextGenerationChunk> for TextGenerationTaskCodec {
    fn capability(&self) -> Capability {
        Capability::TextGeneration
    }

    fn decode_result(&self, payload: Payload) -> Result<TextGenerationResponse, CoreError> {
        decode_text_generation_response(payload)
    }

    fn decode_chunk(&self, chunk: StreamChunk) -> Result<Option<TextGenerationChunk>, CoreError> {
        decode_text_generation_chunk(chunk)
    }
}

struct AudioTranscriptionTaskCodec {
    fallback_language: Option<String>,
}

impl TaskCodec<AudioTranscriptionResponse, Infallible> for AudioTranscriptionTaskCodec {
    fn capability(&self) -> Capability {
        Capability::AudioTranscription
    }

    fn decode_result(&self, payload: Payload) -> Result<AudioTranscriptionResponse, CoreError> {
        decode_audio_transcription_response(payload, self.fallback_language.clone())
    }

    fn decode_chunk(&self, chunk: StreamChunk) -> Result<Option<Infallible>, CoreError> {
        Err(unexpected_chunk("audio_transcription", chunk))
    }
}

struct ImageGenerationTaskCodec;

impl TaskCodec<ImageGenerationResponse, Infallible> for ImageGenerationTaskCodec {
    fn capability(&self) -> Capability {
        Capability::ImageGeneration
    }

    fn decode_result(&self, payload: Payload) -> Result<ImageGenerationResponse, CoreError> {
        decode_image_generation_response(payload)
    }

    fn decode_chunk(&self, chunk: StreamChunk) -> Result<Option<Infallible>, CoreError> {
        Err(unexpected_chunk("image_generation", chunk))
    }
}

struct InferenceImageTaskCodec;

impl TaskCodec<Vec<DiffusionImage>, Infallible> for InferenceImageTaskCodec {
    fn capability(&self) -> Capability {
        Capability::ImageGeneration
    }

    fn decode_result(&self, payload: Payload) -> Result<Vec<DiffusionImage>, CoreError> {
        payload.to_typed().map_err(|error| CoreError::ResultDecodeFailed {
            task_kind: "inference_image".to_owned(),
            message: format!("invalid typed inference image result: {error}"),
        })
    }

    fn decode_chunk(&self, chunk: StreamChunk) -> Result<Option<Infallible>, CoreError> {
        Err(unexpected_chunk("inference_image", chunk))
    }
}

struct ImageEmbeddingTaskCodec {
    output_name: String,
}

impl TaskCodec<ImageEmbeddingResponse, Infallible> for ImageEmbeddingTaskCodec {
    fn capability(&self) -> Capability {
        Capability::ImageEmbedding
    }

    fn decode_result(&self, payload: Payload) -> Result<ImageEmbeddingResponse, CoreError> {
        decode_image_embedding_response(payload, &self.output_name)
    }

    fn decode_chunk(&self, chunk: StreamChunk) -> Result<Option<Infallible>, CoreError> {
        Err(unexpected_chunk("image_embedding", chunk))
    }
}

async fn submit_plan<R, C>(
    execution: &ExecutionHub,
    plan: InvocationPlan,
    codec: impl TaskCodec<R, C>,
) -> Result<TaskHandle<R, C>, CoreError>
where
    R: Send + 'static,
    C: Send + 'static,
{
    let task_id = submit_invocation_plan(execution, plan).await?;
    Ok(TaskHandle::new(execution.orchestrator(), task_id, Arc::new(codec)))
}

async fn submit_invocation_plan(execution: &ExecutionHub, plan: InvocationPlan) -> Result<u64, CoreError> {
    let op = BackendOp { name: plan.invocation.op_name.clone(), options: plan.op_options };

    let mut builder = PipelineBuilder::new(execution.orchestrator(), plan.initial_payload);
    for stage in plan.preprocess_stages {
        builder = builder.cpu_stage(stage);
    }

    if plan.streaming {
        builder
            .gpu_stream(
                plan.invocation.op_name.clone(),
                plan.invocation.backend.backend_id.clone(),
                op,
            )
            .run_stream()
            .await
    } else {
        builder
            .gpu(plan.invocation.op_name.clone(), plan.invocation.backend.backend_id.clone(), op)
            .run()
            .await
    }
}

fn unexpected_chunk(task_kind: &str, chunk: StreamChunk) -> CoreError {
    CoreError::ResultDecodeFailed {
        task_kind: task_kind.to_owned(),
        message: format!("unexpected stream chunk for unary task: {chunk:?}"),
    }
}

#[cfg(feature = "ggml")]
fn decode_wav_payload(path: &std::path::Path) -> Result<Payload, CoreError> {
    crate::infra::backends::ggml::audio_utils::load_pcm_from_wav(&path.to_string_lossy())
        .map(Payload::from)
}

#[cfg(not(feature = "ggml"))]
fn decode_wav_payload(_path: &std::path::Path) -> Result<Payload, CoreError> {
    Err(CoreError::UnsupportedOperation {
        backend: "audio".to_owned(),
        op: "decode_wav".to_owned(),
    })
}
