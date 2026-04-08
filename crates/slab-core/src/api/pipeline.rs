use std::convert::Infallible;
use std::sync::Arc;

use futures::stream::BoxStream;
use slab_diffusion::{Image as DiffusionImage, ImgParams as DiffusionImgParams};
use slab_model_pack::ModelPackRuntimeBridge;
use tokio::sync::Mutex;

use crate::base::error::CoreError;
use crate::base::types::{Payload, StreamChunk};
use crate::inference::{
    AudioTranscriptionRequest, AudioTranscriptionResponse, ImageEmbeddingRequest,
    ImageEmbeddingResponse, ImageGenerationRequest, ImageGenerationResponse, TextGenerationChunk,
    TextGenerationRequest, TextGenerationResponse,
};
use crate::internal::dispatch::{InvocationPlan, ResolvedDriver};
use crate::internal::scheduler::backend::protocol::BackendOp;
use crate::internal::scheduler::pipeline::PipelineBuilder;
use crate::internal::scheduler::stage::CpuStage;
use crate::model::{Capability, ModelSpec};

use super::codec::{
    decode_audio_transcription_response, decode_image_embedding_response,
    decode_image_generation_response, decode_text_generation_chunk,
    decode_text_generation_response, encode_audio_transcription_options,
    encode_image_embedding_request, encode_image_generation_request, encode_load_payload,
    encode_model_pack_load_payload, encode_text_generation_request, image_embedding_input_name,
    image_embedding_output_name,
};
use super::runtime::Runtime;
use super::task::{TaskCodec, TaskHandle};

#[derive(Clone, Debug)]
pub struct Pipeline {
    runtime: Runtime,
    spec: Arc<ModelSpec>,
    pack_bridge: Option<Arc<ModelPackRuntimeBridge>>,
    deployment: Arc<Mutex<Option<LoadedDeployment>>>,
}

#[derive(Clone, Debug)]
struct LoadedDeployment {
    resolved: ResolvedDriver,
}

impl Pipeline {
    pub(crate) fn new(runtime: Runtime, spec: ModelSpec) -> Result<Self, CoreError> {
        Ok(Self {
            runtime,
            spec: Arc::new(spec),
            pack_bridge: None,
            deployment: Arc::new(Mutex::new(None)),
        })
    }

    pub(crate) fn new_with_model_pack_bridge(
        runtime: Runtime,
        bridge: ModelPackRuntimeBridge,
    ) -> Result<Self, CoreError> {
        let mut spec = bridge.model_spec.clone();
        let preferred_driver = bridge.backend.canonical_id().to_owned();
        if !spec.driver_hints.prefer_drivers.iter().any(|driver| driver == &preferred_driver) {
            spec.driver_hints.prefer_drivers.insert(0, preferred_driver);
        }

        Ok(Self {
            runtime,
            spec: Arc::new(spec),
            pack_bridge: Some(Arc::new(bridge)),
            deployment: Arc::new(Mutex::new(None)),
        })
    }

    pub fn model(&self) -> &ModelSpec {
        self.spec.as_ref()
    }

    pub fn model_pack_bridge(&self) -> Option<&ModelPackRuntimeBridge> {
        self.pack_bridge.as_deref()
    }

    pub fn capability(&self) -> Capability {
        self.spec.capability
    }

    pub async fn load(&self) -> Result<(), CoreError> {
        let _ = self
            .ensure_loaded_for(self.spec.capability, self.spec.driver_hints.require_streaming)
            .await?;
        Ok(())
    }

    pub async fn unload(&self) -> Result<(), CoreError> {
        let resolved = {
            let guard = self.deployment.lock().await;
            guard.as_ref().map(|deployment| deployment.resolved.clone())
        };

        if let Some(resolved) = resolved {
            self.runtime.orchestrator().unload_model_backend(&resolved.backend_id).await?;
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
        submit_plan(&self.runtime, plan, TextGenerationTaskCodec).await
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
            vec![CpuStage::new("audio.decode.wav", move |_| {
                crate::internal::engine::audio_utils::load_pcm_from_wav(&path.to_string_lossy())
                    .map(Payload::from)
            })]
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
            &self.runtime,
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
        submit_plan(&self.runtime, plan, ImageGenerationTaskCodec).await
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
        submit_plan(&self.runtime, plan, InferenceImageTaskCodec).await
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
        submit_plan(&self.runtime, plan, ImageEmbeddingTaskCodec { output_name }).await
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

        let resolved =
            self.runtime.resolver().resolve(self.spec.as_ref(), capability, streaming)?;
        let payload = match self.pack_bridge.as_deref() {
            Some(bridge) if resolved_matches_model_pack_bridge(&resolved, bridge) => {
                encode_model_pack_load_payload(bridge)?
            }
            _ => encode_load_payload(self.spec.as_ref(), &resolved)?,
        };
        self.runtime.orchestrator().load_model_backend(&resolved.backend_id, payload).await?;

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

fn resolved_matches_model_pack_bridge(
    resolved: &ResolvedDriver,
    bridge: &ModelPackRuntimeBridge,
) -> bool {
    let canonical_backend = bridge.backend.canonical_id();
    resolved.driver_id == canonical_backend
        || resolved.backend_id == canonical_backend
        || (canonical_backend == "onnx" && resolved.driver_id.starts_with("onnx."))
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
    runtime: &Runtime,
    plan: InvocationPlan,
    codec: impl TaskCodec<R, C>,
) -> Result<TaskHandle<R, C>, CoreError>
where
    R: Send + 'static,
    C: Send + 'static,
{
    let task_id = submit_invocation_plan(runtime, plan).await?;
    Ok(TaskHandle::new(runtime.orchestrator(), task_id, Arc::new(codec)))
}

async fn submit_invocation_plan(runtime: &Runtime, plan: InvocationPlan) -> Result<u64, CoreError> {
    let op = BackendOp { name: plan.invocation.op_name.clone(), options: plan.op_options };

    let mut builder = PipelineBuilder::new(runtime.orchestrator(), plan.initial_payload);
    for stage in plan.preprocess_stages {
        builder = builder.cpu_stage(stage);
    }

    if plan.streaming {
        builder
            .gpu_stream(
                plan.invocation.op_name.clone(),
                plan.invocation.driver.backend_id.clone(),
                op,
            )
            .run_stream()
            .await
    } else {
        builder
            .gpu(plan.invocation.op_name.clone(), plan.invocation.driver.backend_id.clone(), op)
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use base64::Engine as _;
    use futures::StreamExt;
    use image::{ColorType, GenericImageView, ImageEncoder, Rgb, RgbImage};
    use serde_json::json;
    use tokio::sync::mpsc;

    use crate::base::types::{Payload, StreamChunk};
    use crate::internal::dispatch::{
        DriverDescriptor, DriverLoadStyle, DriverResolver, ModelSourceKind,
    };
    use crate::internal::scheduler::backend::admission::{ResourceManager, ResourceManagerConfig};
    use crate::internal::scheduler::backend::protocol::{
        BackendReply, DriverRequestKind, ManagementEvent, RequestRoute,
    };
    use crate::internal::scheduler::orchestrator::Orchestrator;

    use super::super::runtime::{DriversConfig, Runtime};
    use super::super::task::TaskState;
    use super::*;
    use crate::base::error::CoreError;
    use crate::model::{Capability, ModelFamily, ModelSource};

    fn text_spec() -> ModelSpec {
        ModelSpec::new(
            ModelFamily::Llama,
            Capability::TextGeneration,
            ModelSource::LocalPath { path: PathBuf::from("fixtures/fake-model.gguf") },
        )
    }

    fn audio_spec() -> ModelSpec {
        ModelSpec::new(
            ModelFamily::Whisper,
            Capability::AudioTranscription,
            ModelSource::LocalPath { path: PathBuf::from("fixtures/fake-model.bin") },
        )
    }

    fn image_spec() -> ModelSpec {
        ModelSpec::new(
            ModelFamily::Diffusion,
            Capability::ImageGeneration,
            ModelSource::LocalPath { path: PathBuf::from("fixtures/fake-model.safetensors") },
        )
    }

    fn embedding_spec() -> ModelSpec {
        ModelSpec::new(
            ModelFamily::Onnx,
            Capability::ImageEmbedding,
            ModelSource::LocalPath { path: PathBuf::from("fixtures/fake-model.onnx") },
        )
        .with_metadata("input_tensor_name", "image_input")
        .with_metadata("output_tensor_name", "embedding_out")
    }

    #[derive(Clone)]
    enum MockBackend {
        Text,
        Audio,
        Image,
        Embedding { input_name: &'static str },
    }

    fn register_mock_backend(
        resource_manager: &mut ResourceManager,
        backend_id: &'static str,
        backend: MockBackend,
    ) {
        resource_manager.register_backend(backend_id, move |shared_rx, _control_tx| {
            let backend = backend.clone();
            tokio::spawn(async move {
                let mut loaded = false;

                loop {
                    let req = {
                        let mut lock = shared_rx.lock().await;
                        lock.recv().await
                    };

                    let Some(req) = req else {
                        break;
                    };

                    match req.driver_kind().expect("backend request should be typed") {
                        DriverRequestKind::Management { event, .. } => {
                            match event {
                                ManagementEvent::LoadModel => loaded = true,
                                ManagementEvent::UnloadModel => loaded = false,
                            }
                            let _ = req.reply_tx.send(BackendReply::Value(Payload::None));
                        }
                        DriverRequestKind::Inference(invocation) => {
                            if !loaded {
                                let _ = req
                                    .reply_tx
                                    .send(BackendReply::Error("model not loaded".to_owned()));
                                continue;
                            }

                            match &backend {
                                MockBackend::Text => match invocation.route {
                                    RequestRoute::Inference => {
                                        let text = req
                                            .input
                                            .to_str()
                                            .map(str::to_owned)
                                            .expect("text generation input should be text");

                                        if text == "__wait__" {
                                            let mut cancel_rx = req.cancel_rx.clone();
                                            let _ = cancel_rx.wait_for(|cancelled| *cancelled).await;
                                            let _ = req
                                                .reply_tx
                                                .send(BackendReply::Error("cancelled".into()));
                                        } else {
                                            let _ = req.reply_tx.send(BackendReply::Value(
                                                Payload::text(text),
                                            ));
                                        }
                                    }
                                    RequestRoute::InferenceStream => {
                                        let text = req
                                            .input
                                            .to_str()
                                            .map(str::to_owned)
                                            .expect("streaming input should be text");
                                        let (stream_tx, stream_rx) = mpsc::channel(4);
                                        let _ = req.reply_tx.send(BackendReply::Stream(stream_rx));

                                        if text == "__stream_wait__" {
                                            let mut cancel_rx = req.cancel_rx.clone();
                                            tokio::spawn(async move {
                                                let _ =
                                                    stream_tx.send(StreamChunk::Token("tick".into())).await;
                                                let _ = cancel_rx.wait_for(|cancelled| *cancelled).await;
                                            });
                                        } else {
                                            tokio::spawn(async move {
                                                let _ = stream_tx.send(StreamChunk::Token(text)).await;
                                                let _ = stream_tx.send(StreamChunk::Done).await;
                                            });
                                        }
                                    }
                                    other => {
                                        let _ = req.reply_tx.send(BackendReply::Error(format!(
                                            "unsupported route for text backend: {other:?}"
                                        )));
                                    }
                                },
                                MockBackend::Audio => match invocation.route {
                                    RequestRoute::Inference => {
                                        let samples = req
                                            .input
                                            .to_f32_arc()
                                            .expect("audio preprocess should produce f32 PCM");
                                        let _ = req.reply_tx.send(BackendReply::Value(Payload::json(
                                            json!({ "text": format!("decoded {} samples", samples.len()) }),
                                        )));
                                    }
                                    other => {
                                        let _ = req.reply_tx.send(BackendReply::Error(format!(
                                            "unsupported route for audio backend: {other:?}"
                                        )));
                                    }
                                },
                                MockBackend::Image => match invocation.route {
                                    RequestRoute::InferenceImage => {
                                        let body: DiffusionImgParams = req
                                            .input
                                            .to_typed()
                                            .expect("image request should be typed diffusion input");
                                        assert_eq!(body.prompt.as_deref(), Some("generate a cat"));
                                        let width =
                                            body.width.expect("mock image width should be set");
                                        let height =
                                            body.height.expect("mock image height should be set");
                                        let pixel_count = (width as usize)
                                            .saturating_mul(height as usize)
                                            .saturating_mul(3);
                                        let _ = req.reply_tx.send(BackendReply::Value(
                                            Payload::typed(vec![DiffusionImage {
                                                width,
                                                height,
                                                channel: 3,
                                                data: vec![255; pixel_count],
                                            }]),
                                        ));
                                    }
                                    other => {
                                        let _ = req.reply_tx.send(BackendReply::Error(format!(
                                            "unsupported route for image backend: {other:?}"
                                        )));
                                    }
                                },
                                MockBackend::Embedding { input_name } => match invocation.route {
                                    RequestRoute::Inference => {
                                        let body: serde_json::Value = req
                                            .input
                                            .to_json()
                                            .expect("embedding request should be JSON");
                                        assert!(
                                            body.get("inputs")
                                                .and_then(|value| value.get(*input_name))
                                                .is_some(),
                                            "encoded image tensor should use the configured input tensor name"
                                        );

                                        let embedding = [0.25f32, 0.5f32, 0.75f32];
                                        let raw: Vec<u8> = embedding
                                            .iter()
                                            .flat_map(|value| value.to_le_bytes())
                                            .collect();
                                        let encoded = base64::engine::general_purpose::STANDARD
                                            .encode(raw);

                                        let _ = req.reply_tx.send(BackendReply::Value(Payload::json(
                                            json!({
                                                "outputs": {
                                                    "embedding_out": {
                                                        "data_b64": encoded,
                                                    }
                                                }
                                            }),
                                        )));
                                    }
                                    other => {
                                        let _ = req.reply_tx.send(BackendReply::Error(format!(
                                            "unsupported route for embedding backend: {other:?}"
                                        )));
                                    }
                                },
                            }
                        }
                    }
                }
            });
        });
    }

    fn test_runtime() -> Runtime {
        let mut resource_manager = ResourceManager::with_config(ResourceManagerConfig {
            backend_capacity: 4,
            ..ResourceManagerConfig::default()
        });
        register_mock_backend(&mut resource_manager, "test-llama", MockBackend::Text);
        register_mock_backend(&mut resource_manager, "test-whisper", MockBackend::Audio);
        register_mock_backend(&mut resource_manager, "test-diffusion", MockBackend::Image);
        register_mock_backend(
            &mut resource_manager,
            "test-embedding",
            MockBackend::Embedding { input_name: "image_input" },
        );

        let supported_sources = vec![
            ModelSourceKind::LocalPath,
            ModelSourceKind::LocalArtifacts,
            ModelSourceKind::HuggingFace,
        ];
        let resolver = DriverResolver::new(vec![
            DriverDescriptor {
                driver_id: "candle.llama".to_owned(),
                backend_id: "test-llama".to_owned(),
                family: ModelFamily::Llama,
                capability: Capability::TextGeneration,
                supported_sources: supported_sources.clone(),
                supports_streaming: true,
                load_style: DriverLoadStyle::ModelOnly,
                priority: 0,
            },
            DriverDescriptor {
                driver_id: "candle.whisper".to_owned(),
                backend_id: "test-whisper".to_owned(),
                family: ModelFamily::Whisper,
                capability: Capability::AudioTranscription,
                supported_sources: supported_sources.clone(),
                supports_streaming: false,
                load_style: DriverLoadStyle::ModelOnly,
                priority: 0,
            },
            DriverDescriptor {
                driver_id: "candle.diffusion".to_owned(),
                backend_id: "test-diffusion".to_owned(),
                family: ModelFamily::Diffusion,
                capability: Capability::ImageGeneration,
                supported_sources: supported_sources.clone(),
                supports_streaming: false,
                load_style: DriverLoadStyle::ModelOnly,
                priority: 0,
            },
            DriverDescriptor {
                driver_id: "onnx.embedding".to_owned(),
                backend_id: "test-embedding".to_owned(),
                family: ModelFamily::Onnx,
                capability: Capability::ImageEmbedding,
                supported_sources,
                supports_streaming: false,
                load_style: DriverLoadStyle::ModelOnly,
                priority: 0,
            },
        ]);

        Runtime::new(Orchestrator::start(resource_manager, 32), resolver, DriversConfig::default())
    }

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("slab-core-pipeline-{}-{nanos}-{name}", std::process::id()))
    }

    fn write_test_wav(samples: &[i16]) -> PathBuf {
        let path = unique_temp_path("audio.wav");
        let num_channels = 1u16;
        let sample_rate = 16_000u32;
        let bits_per_sample = 16u16;
        let block_align = num_channels * (bits_per_sample / 8);
        let byte_rate = sample_rate * u32::from(block_align);
        let data_len = u32::try_from(std::mem::size_of_val(samples))
            .expect("test wav data length should fit in u32");
        let riff_len = 36 + data_len;

        let mut bytes = Vec::with_capacity((44 + data_len) as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&riff_len.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&num_channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        bytes.extend_from_slice(&block_align.to_le_bytes());
        bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }

        std::fs::write(&path, bytes).expect("test wav should be written");
        path
    }

    fn sample_png_bytes() -> Vec<u8> {
        let image = RgbImage::from_pixel(2, 2, Rgb([255, 0, 0]));
        let mut bytes = Cursor::new(Vec::new());
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(image.as_raw(), image.width(), image.height(), ColorType::Rgb8.into())
            .expect("png should encode");
        bytes.into_inner()
    }

    #[tokio::test]
    async fn pipeline_runs_and_streams_text_generation() {
        let runtime = test_runtime();
        let pipeline = runtime.pipeline(text_spec()).expect("pipeline should build");

        pipeline.load().await.expect("model should load");

        let response = pipeline
            .run_text_generation(TextGenerationRequest {
                prompt: "hello runtime".to_owned(),
                ..TextGenerationRequest::default()
            })
            .await
            .expect("run should succeed");
        assert_eq!(response.text, "hello runtime");

        let chunks = pipeline
            .stream_text_generation(TextGenerationRequest {
                prompt: "hello stream".to_owned(),
                ..TextGenerationRequest::default()
            })
            .await
            .expect("stream should start")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("stream chunks should decode");

        let combined = chunks.into_iter().map(|chunk| chunk.delta).collect::<Vec<_>>().join("");
        assert_eq!(combined, "hello stream");
    }

    #[tokio::test]
    async fn runtime_builds_pipeline_from_model_pack_bridge() {
        let runtime = test_runtime();
        let bridge = slab_model_pack::ModelPackRuntimeBridge {
            backend: slab_types::RuntimeBackendId::CandleLlama,
            capability: Capability::TextGeneration,
            model_spec: text_spec(),
            load_defaults: slab_model_pack::ModelPackLoadDefaults::default(),
            inference_defaults: Default::default(),
        };

        let pipeline = runtime
            .pipeline_from_model_pack(bridge.clone())
            .expect("model pack pipeline should build");

        assert_eq!(
            pipeline.model().source.primary_path().map(|path| path.to_string_lossy().to_string()),
            Some("fixtures/fake-model.gguf".to_owned())
        );
        assert_eq!(
            pipeline.model_pack_bridge().map(|value| value.backend),
            Some(slab_types::RuntimeBackendId::CandleLlama)
        );
        assert_eq!(
            pipeline.model().driver_hints.prefer_drivers.first().map(String::as_str),
            Some("candle.llama")
        );
    }

    #[tokio::test]
    async fn submit_returns_task_handle_with_lifecycle_controls() {
        let runtime = test_runtime();
        let pipeline = runtime.pipeline(text_spec()).expect("pipeline should build");

        let handle = pipeline
            .submit_text_generation(TextGenerationRequest {
                prompt: "hello task".to_owned(),
                ..TextGenerationRequest::default()
            })
            .await
            .expect("task submission should succeed");

        let snapshot = handle.status().await.expect("status should be readable");
        assert_eq!(snapshot.capability, Capability::TextGeneration);

        let response = handle.result().await.expect("task should succeed");
        assert_eq!(response.text, "hello task");

        let snapshot = handle.status().await.expect("status should still be readable after result");
        assert!(matches!(snapshot.status, TaskState::ResultConsumed));

        handle.purge().await;

        let error = handle.status().await.expect_err("purged task should be gone");
        assert!(matches!(error, CoreError::TaskNotFound { .. }));
    }

    #[tokio::test]
    async fn submit_stream_exposes_stream_and_cancel_and_purge() {
        let runtime = test_runtime();
        let pipeline = runtime.pipeline(text_spec()).expect("pipeline should build");

        let stream_handle = pipeline
            .submit_text_generation(TextGenerationRequest {
                prompt: "stream via task handle".to_owned(),
                stream: true,
                ..TextGenerationRequest::default()
            })
            .await
            .expect("streaming task should submit");

        let stream_chunks = stream_handle
            .take_stream()
            .await
            .expect("stream handle should be available")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("stream chunks should decode");

        let combined =
            stream_chunks.into_iter().map(|chunk| chunk.delta).collect::<Vec<_>>().join("");
        assert_eq!(combined, "stream via task handle");

        let slow_handle = pipeline
            .submit_text_generation(TextGenerationRequest {
                prompt: "__wait__".to_owned(),
                ..TextGenerationRequest::default()
            })
            .await
            .expect("slow task should submit");

        slow_handle.cancel();
        slow_handle.cancel_and_purge().await;

        let error =
            slow_handle.status().await.expect_err("cancelled and purged task should be gone");
        assert!(matches!(error, CoreError::TaskNotFound { .. }));
    }

    #[tokio::test]
    async fn cancel_closes_active_streaming_task() {
        let runtime = test_runtime();
        let pipeline = runtime.pipeline(text_spec()).expect("pipeline should build");

        let stream_handle = pipeline
            .submit_text_generation(TextGenerationRequest {
                prompt: "__stream_wait__".to_owned(),
                stream: true,
                ..TextGenerationRequest::default()
            })
            .await
            .expect("streaming task should submit");

        let mut stream =
            stream_handle.take_stream().await.expect("stream handle should be available");

        let first_chunk = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .expect("stream should yield an initial chunk")
            .expect("stream should not close before cancel")
            .expect("initial chunk should decode");
        assert_eq!(first_chunk.delta, "tick");

        stream_handle.cancel();

        let next_chunk = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .expect("stream should react to cancellation");
        assert!(
            next_chunk.is_none(),
            "stream should close after the task is cancelled, got {next_chunk:?}"
        );

        stream_handle.cancel_and_purge().await;
    }

    #[tokio::test]
    async fn pipeline_runs_audio_transcription_with_wav_preprocess() {
        let runtime = test_runtime();
        let pipeline = runtime.pipeline(audio_spec()).expect("pipeline should build");
        let audio_path = write_test_wav(&[0, 1024, -1024, 2048]);

        let response = pipeline
            .run_audio_transcription(AudioTranscriptionRequest {
                audio_path: audio_path.clone(),
                pcm_samples: None,
                language: Some("zh".to_owned()),
                prompt: None,
                vad: None,
                decode: None,
                options: Default::default(),
            })
            .await
            .expect("audio transcription should succeed");

        std::fs::remove_file(&audio_path).expect("temporary audio file should be removed");

        assert_eq!(response.text, "decoded 4 samples");
        assert_eq!(response.language.as_deref(), Some("zh"));
    }

    #[tokio::test]
    async fn pipeline_runs_audio_transcription_with_preloaded_pcm() {
        let runtime = test_runtime();
        let pipeline = runtime.pipeline(audio_spec()).expect("pipeline should build");

        let response = pipeline
            .run_audio_transcription(AudioTranscriptionRequest {
                audio_path: std::path::PathBuf::from("fixtures/nonexistent.wav"),
                pcm_samples: Some(std::sync::Arc::<[f32]>::from(vec![0.0, 0.5, -0.5, 1.0])),
                language: Some("en".to_owned()),
                prompt: None,
                vad: None,
                decode: None,
                options: Default::default(),
            })
            .await
            .expect("audio transcription with preloaded PCM should succeed");

        assert_eq!(response.text, "decoded 4 samples");
        assert_eq!(response.language.as_deref(), Some("en"));
    }

    #[tokio::test]
    async fn pipeline_runs_image_generation() {
        let runtime = test_runtime();
        let pipeline = runtime.pipeline(image_spec()).expect("pipeline should build");

        let response = pipeline
            .run_image_generation(ImageGenerationRequest {
                prompt: "generate a cat".to_owned(),
                width: 256,
                height: 256,
                steps: Some(4),
                guidance: Some(6.5),
                ..ImageGenerationRequest::default()
            })
            .await
            .expect("image generation should succeed");

        assert_eq!(response.images.len(), 1);
        let decoded =
            image::load_from_memory(&response.images[0]).expect("generated image should be a PNG");
        assert_eq!(decoded.dimensions(), (256, 256));
    }

    #[tokio::test]
    async fn pipeline_runs_canonical_inference_image() {
        let runtime = test_runtime();
        let pipeline = runtime.pipeline(image_spec()).expect("pipeline should build");

        let response = pipeline
            .run_inference_image(DiffusionImgParams {
                prompt: Some("generate a cat".to_owned()),
                width: Some(128),
                height: Some(64),
                ..Default::default()
            })
            .await
            .expect("canonical inference image should succeed");

        assert_eq!(response.len(), 1);
        assert_eq!(response[0].width, 128);
        assert_eq!(response[0].height, 64);
        assert_eq!(response[0].channel, 3);
    }

    #[tokio::test]
    async fn pipeline_runs_image_embedding() {
        let runtime = test_runtime();
        let pipeline = runtime.pipeline(embedding_spec()).expect("pipeline should build");

        let response = pipeline
            .run_image_embedding(ImageEmbeddingRequest {
                image: sample_png_bytes(),
                options: Default::default(),
            })
            .await
            .expect("image embedding should succeed");

        assert_eq!(response.embedding, vec![0.25, 0.5, 0.75]);
    }

    #[tokio::test]
    async fn result_timeout_cancels_and_purges_task() {
        let runtime = test_runtime();
        let pipeline = runtime.pipeline(text_spec()).expect("pipeline should build");

        let handle = pipeline
            .submit_text_generation(TextGenerationRequest {
                prompt: "__wait__".to_owned(),
                ..TextGenerationRequest::default()
            })
            .await
            .expect("slow task should submit");

        let error = handle
            .result_timeout(Duration::from_millis(20))
            .await
            .expect_err("timeout should surface as a core error");
        assert!(matches!(error, CoreError::Timeout));

        let status_error = handle.status().await.expect_err("timed out task should be purged");
        assert!(matches!(status_error, CoreError::TaskNotFound { .. }));
    }
}
