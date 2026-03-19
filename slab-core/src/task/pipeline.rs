use std::sync::Arc;

use futures::stream::BoxStream;

use crate::base::error::CoreError;
use crate::base::types::{Payload, StreamChunk};
use crate::dispatch::ExecutionPlan;
use crate::model::{
    AutoModel, AutoModelForAudioTranscription, AutoModelForImageEmbedding,
    AutoModelForImageGeneration, AutoModelForTextGeneration,
};
use crate::runtime::Runtime;
use crate::scheduler::backend::protocol::BackendOp;
use crate::scheduler::pipeline::PipelineBuilder;
use crate::scheduler::stage::CpuStage;
use crate::task::codec::{
    decode_audio_transcription_response, decode_image_embedding_response,
    decode_image_generation_response, decode_text_generation_chunk,
    decode_text_generation_response, encode_audio_transcription_options,
    encode_image_embedding_request, encode_image_generation_request,
    encode_text_generation_request, image_embedding_input_name, image_embedding_output_name,
};
use crate::spec::{
    AudioTranscriptionRequest, AudioTranscriptionResponse, Capability, ImageEmbeddingRequest,
    ImageEmbeddingResponse, ImageGenerationRequest, ImageGenerationResponse, ModelSpec, TaskKind,
    TextGenerationChunk, TextGenerationRequest, TextGenerationResponse,
};
use crate::task::handle::{NeverChunk, TaskCodec, TaskHandle};

#[derive(Clone, Debug)]
pub enum Pipeline {
    TextGeneration(TextGenerationPipeline),
    AudioTranscription(AudioTranscriptionPipeline),
    ImageGeneration(ImageGenerationPipeline),
    ImageEmbedding(ImageEmbeddingPipeline),
}

impl Pipeline {
    pub fn task_kind(&self) -> TaskKind {
        match self {
            Self::TextGeneration(_) => TaskKind::TextGeneration,
            Self::AudioTranscription(_) => TaskKind::AudioTranscription,
            Self::ImageGeneration(_) => TaskKind::ImageGeneration,
            Self::ImageEmbedding(_) => TaskKind::ImageEmbedding,
        }
    }

    pub fn into_text_generation(self) -> Option<TextGenerationPipeline> {
        match self {
            Self::TextGeneration(pipeline) => Some(pipeline),
            _ => None,
        }
    }

    pub fn into_audio_transcription(self) -> Option<AudioTranscriptionPipeline> {
        match self {
            Self::AudioTranscription(pipeline) => Some(pipeline),
            _ => None,
        }
    }

    pub fn into_image_generation(self) -> Option<ImageGenerationPipeline> {
        match self {
            Self::ImageGeneration(pipeline) => Some(pipeline),
            _ => None,
        }
    }

    pub fn into_image_embedding(self) -> Option<ImageEmbeddingPipeline> {
        match self {
            Self::ImageEmbedding(pipeline) => Some(pipeline),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum PipelineModelInput {
    Model(AutoModel),
    Spec(ModelSpec),
}

impl From<AutoModel> for PipelineModelInput {
    fn from(value: AutoModel) -> Self {
        Self::Model(value)
    }
}

impl From<&AutoModel> for PipelineModelInput {
    fn from(value: &AutoModel) -> Self {
        Self::Model(value.clone())
    }
}

impl From<ModelSpec> for PipelineModelInput {
    fn from(value: ModelSpec) -> Self {
        Self::Spec(value)
    }
}

impl From<&ModelSpec> for PipelineModelInput {
    fn from(value: &ModelSpec) -> Self {
        Self::Spec(value.clone())
    }
}

impl From<AutoModelForTextGeneration> for PipelineModelInput {
    fn from(value: AutoModelForTextGeneration) -> Self {
        Self::Model(value.into())
    }
}

impl From<&AutoModelForTextGeneration> for PipelineModelInput {
    fn from(value: &AutoModelForTextGeneration) -> Self {
        Self::Model(value.model.clone())
    }
}

impl From<AutoModelForAudioTranscription> for PipelineModelInput {
    fn from(value: AutoModelForAudioTranscription) -> Self {
        Self::Model(value.into())
    }
}

impl From<&AutoModelForAudioTranscription> for PipelineModelInput {
    fn from(value: &AutoModelForAudioTranscription) -> Self {
        Self::Model(value.model.clone())
    }
}

impl From<AutoModelForImageGeneration> for PipelineModelInput {
    fn from(value: AutoModelForImageGeneration) -> Self {
        Self::Model(value.into())
    }
}

impl From<&AutoModelForImageGeneration> for PipelineModelInput {
    fn from(value: &AutoModelForImageGeneration) -> Self {
        Self::Model(value.model.clone())
    }
}

impl From<AutoModelForImageEmbedding> for PipelineModelInput {
    fn from(value: AutoModelForImageEmbedding) -> Self {
        Self::Model(value.into())
    }
}

impl From<&AutoModelForImageEmbedding> for PipelineModelInput {
    fn from(value: &AutoModelForImageEmbedding) -> Self {
        Self::Model(value.model.clone())
    }
}

pub fn pipeline(
    runtime: &Runtime,
    task_kind: TaskKind,
    model: impl Into<PipelineModelInput>,
) -> Result<Pipeline, CoreError> {
    let model = match model.into() {
        PipelineModelInput::Model(model) => model,
        PipelineModelInput::Spec(spec) => runtime.model(spec),
    };

    Ok(match task_kind {
        TaskKind::TextGeneration => Pipeline::TextGeneration(TextGenerationPipeline::new(model)?),
        TaskKind::AudioTranscription => {
            Pipeline::AudioTranscription(AudioTranscriptionPipeline::new(model)?)
        }
        TaskKind::ImageGeneration => {
            Pipeline::ImageGeneration(ImageGenerationPipeline::new(model)?)
        }
        TaskKind::ImageEmbedding => {
            Pipeline::ImageEmbedding(ImageEmbeddingPipeline::new(model)?)
        }
    })
}

#[derive(Clone, Debug)]
pub struct TextGenerationPipeline {
    model: AutoModel,
}

impl TextGenerationPipeline {
    pub fn new(model: AutoModel) -> Result<Self, CoreError> {
        require_capability(&model, Capability::TextGeneration)?;
        Ok(Self { model })
    }

    pub async fn run(
        &self,
        mut request: TextGenerationRequest,
    ) -> Result<TextGenerationResponse, CoreError> {
        request.stream = false;
        self.submit(request).await?.result().await
    }

    pub async fn stream(
        &self,
        mut request: TextGenerationRequest,
    ) -> Result<BoxStream<'static, Result<TextGenerationChunk, CoreError>>, CoreError> {
        request.stream = true;
        self.submit(request).await?.take_stream().await
    }

    pub async fn submit(
        &self,
        request: TextGenerationRequest,
    ) -> Result<TaskHandle<TextGenerationResponse, TextGenerationChunk>, CoreError> {
        let deployment = self
            .model
            .ensure_loaded_for(TaskKind::TextGeneration, request.stream)
            .await?;
        let resolved = resolved_for_request(&deployment.resolved, TaskKind::TextGeneration, request.stream)?;
        let (input, op_options) = encode_text_generation_request(&request, &resolved)?;
        let plan = ExecutionPlan {
            resolved,
            initial_payload: input,
            preprocess_stages: Vec::new(),
            op_options,
            streaming: request.stream,
        };
        submit_plan(self.model.runtime(), plan, TextGenerationTaskCodec).await
    }
}

#[derive(Clone, Debug)]
pub struct AudioTranscriptionPipeline {
    model: AutoModel,
}

impl AudioTranscriptionPipeline {
    pub fn new(model: AutoModel) -> Result<Self, CoreError> {
        require_capability(&model, Capability::AudioTranscription)?;
        Ok(Self { model })
    }

    pub async fn run(
        &self,
        request: AudioTranscriptionRequest,
    ) -> Result<AudioTranscriptionResponse, CoreError> {
        self.submit(request).await?.result().await
    }

    pub async fn submit(
        &self,
        request: AudioTranscriptionRequest,
    ) -> Result<TaskHandle<AudioTranscriptionResponse, NeverChunk>, CoreError> {
        let deployment = self
            .model
            .ensure_loaded_for(TaskKind::AudioTranscription, false)
            .await?;
        let resolved =
            resolved_for_request(&deployment.resolved, TaskKind::AudioTranscription, false)?;
        let path = request.audio_path.clone();
        let plan = ExecutionPlan {
            resolved,
            initial_payload: Payload::None,
            preprocess_stages: vec![CpuStage::new("audio.decode.wav", move |_| {
                crate::engine::audio_utils::load_pcm_from_wav(&path.to_string_lossy())
                    .map(Payload::from)
                    .map_err(|error| error.to_string())
            })],
            op_options: encode_audio_transcription_options(&request),
            streaming: false,
        };
        submit_plan(
            self.model.runtime(),
            plan,
            AudioTranscriptionTaskCodec {
                fallback_language: request.language.clone(),
            },
        )
        .await
    }
}

#[derive(Clone, Debug)]
pub struct ImageGenerationPipeline {
    model: AutoModel,
}

impl ImageGenerationPipeline {
    pub fn new(model: AutoModel) -> Result<Self, CoreError> {
        require_capability(&model, Capability::ImageGeneration)?;
        Ok(Self { model })
    }

    pub async fn run(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse, CoreError> {
        self.submit(request).await?.result().await
    }

    pub async fn submit(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<TaskHandle<ImageGenerationResponse, NeverChunk>, CoreError> {
        let deployment = self
            .model
            .ensure_loaded_for(TaskKind::ImageGeneration, false)
            .await?;
        let resolved = resolved_for_request(&deployment.resolved, TaskKind::ImageGeneration, false)?;
        let (input, op_options) = encode_image_generation_request(&request, &resolved)?;
        let plan = ExecutionPlan {
            resolved,
            initial_payload: input,
            preprocess_stages: Vec::new(),
            op_options,
            streaming: false,
        };
        submit_plan(self.model.runtime(), plan, ImageGenerationTaskCodec).await
    }
}

#[derive(Clone, Debug)]
pub struct ImageEmbeddingPipeline {
    model: AutoModel,
}

impl ImageEmbeddingPipeline {
    pub fn new(model: AutoModel) -> Result<Self, CoreError> {
        require_capability(&model, Capability::ImageEmbedding)?;
        Ok(Self { model })
    }

    pub async fn run(
        &self,
        request: ImageEmbeddingRequest,
    ) -> Result<ImageEmbeddingResponse, CoreError> {
        self.submit(request).await?.result().await
    }

    pub async fn submit(
        &self,
        request: ImageEmbeddingRequest,
    ) -> Result<TaskHandle<ImageEmbeddingResponse, NeverChunk>, CoreError> {
        let deployment = self
            .model
            .ensure_loaded_for(TaskKind::ImageEmbedding, false)
            .await?;
        let resolved = resolved_for_request(&deployment.resolved, TaskKind::ImageEmbedding, false)?;
        let spec = self.model.spec();
        let input_name = image_embedding_input_name(spec);
        let output_name = image_embedding_output_name(spec);
        let (input, op_options) = encode_image_embedding_request(&request, &input_name)?;
        let plan = ExecutionPlan {
            resolved,
            initial_payload: input,
            preprocess_stages: Vec::new(),
            op_options,
            streaming: false,
        };
        submit_plan(
            self.model.runtime(),
            plan,
            ImageEmbeddingTaskCodec { output_name },
        )
        .await
    }
}

struct TextGenerationTaskCodec;

impl TaskCodec<TextGenerationResponse, TextGenerationChunk> for TextGenerationTaskCodec {
    fn task_kind(&self) -> TaskKind {
        TaskKind::TextGeneration
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

impl TaskCodec<AudioTranscriptionResponse, NeverChunk> for AudioTranscriptionTaskCodec {
    fn task_kind(&self) -> TaskKind {
        TaskKind::AudioTranscription
    }

    fn decode_result(&self, payload: Payload) -> Result<AudioTranscriptionResponse, CoreError> {
        decode_audio_transcription_response(payload, self.fallback_language.clone())
    }

    fn decode_chunk(&self, chunk: StreamChunk) -> Result<Option<NeverChunk>, CoreError> {
        Err(unexpected_chunk(TaskKind::AudioTranscription, chunk))
    }
}

struct ImageGenerationTaskCodec;

impl TaskCodec<ImageGenerationResponse, NeverChunk> for ImageGenerationTaskCodec {
    fn task_kind(&self) -> TaskKind {
        TaskKind::ImageGeneration
    }

    fn decode_result(&self, payload: Payload) -> Result<ImageGenerationResponse, CoreError> {
        decode_image_generation_response(payload)
    }

    fn decode_chunk(&self, chunk: StreamChunk) -> Result<Option<NeverChunk>, CoreError> {
        Err(unexpected_chunk(TaskKind::ImageGeneration, chunk))
    }
}

struct ImageEmbeddingTaskCodec {
    output_name: String,
}

impl TaskCodec<ImageEmbeddingResponse, NeverChunk> for ImageEmbeddingTaskCodec {
    fn task_kind(&self) -> TaskKind {
        TaskKind::ImageEmbedding
    }

    fn decode_result(&self, payload: Payload) -> Result<ImageEmbeddingResponse, CoreError> {
        decode_image_embedding_response(payload, &self.output_name)
    }

    fn decode_chunk(&self, chunk: StreamChunk) -> Result<Option<NeverChunk>, CoreError> {
        Err(unexpected_chunk(TaskKind::ImageEmbedding, chunk))
    }
}

async fn submit_plan<R, C>(
    runtime: &Runtime,
    plan: ExecutionPlan,
    codec: impl TaskCodec<R, C>,
) -> Result<TaskHandle<R, C>, CoreError>
where
    R: Send + 'static,
    C: Send + 'static,
{
    let task_id = submit_execution_plan(runtime, plan).await?;
    Ok(TaskHandle::new(runtime.kernel(), task_id, Arc::new(codec)))
}

async fn submit_execution_plan(runtime: &Runtime, plan: ExecutionPlan) -> Result<u64, CoreError> {
    let op = BackendOp {
        name: plan.resolved.op_name.clone(),
        options: plan.op_options,
    };

    let mut builder = PipelineBuilder::new(runtime.orchestrator(), plan.initial_payload);
    for stage in plan.preprocess_stages {
        builder = builder.cpu_stage(stage);
    }

    if plan.streaming {
        builder
            .gpu_stream(plan.resolved.op_name, plan.resolved.backend_id, op)
            .run_stream()
            .await
    } else {
        builder
            .gpu(plan.resolved.op_name, plan.resolved.backend_id, op)
            .run()
            .await
    }
}

fn require_capability(model: &AutoModel, capability: Capability) -> Result<(), CoreError> {
    if model.spec().capability == capability {
        Ok(())
    } else {
        Err(CoreError::UnsupportedCapability {
            family: format!("{:?}", model.spec().family),
            capability: format!("{:?}", capability),
        })
    }
}

fn unexpected_chunk(task_kind: TaskKind, chunk: StreamChunk) -> CoreError {
    CoreError::ResultDecodeFailed {
        task_kind: format!("{task_kind:?}"),
        message: format!("unexpected stream chunk for unary task: {chunk:?}"),
    }
}

fn resolved_for_request(
    deployment: &crate::dispatch::ResolvedInvocation,
    task_kind: TaskKind,
    streaming: bool,
) -> Result<crate::dispatch::ResolvedInvocation, CoreError> {
    if deployment.capability != task_kind.capability() {
        return Err(CoreError::UnsupportedCapability {
            family: format!("{:?}", deployment.family),
            capability: format!("{:?}", task_kind.capability()),
        });
    }

    if streaming && !deployment.supports_streaming {
        return Err(CoreError::UnsupportedOperation {
            backend: deployment.driver_id.clone(),
            op: "stream".to_owned(),
        });
    }

    let op_name = match (task_kind, streaming) {
        (TaskKind::TextGeneration, true) => "inference.stream",
        (TaskKind::ImageGeneration, _) => "inference.image",
        (TaskKind::TextGeneration, false)
        | (TaskKind::AudioTranscription, _)
        | (TaskKind::ImageEmbedding, _) => "inference",
    };

    let mut resolved = deployment.clone();
    resolved.task_kind = task_kind;
    resolved.op_name = op_name.to_owned();
    Ok(resolved)
}
