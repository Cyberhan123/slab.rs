use std::sync::Arc;

use tokio::sync::Mutex;

use crate::base::error::CoreError;
use crate::model::deployment::ModelDeployment;
use crate::runtime::Runtime;
use crate::spec::{
    AudioTranscriptionRequest, AudioTranscriptionResponse, Capability, ImageEmbeddingRequest,
    ImageEmbeddingResponse, ImageGenerationRequest, ImageGenerationResponse, ModelSpec, TaskKind,
    TextGenerationChunk, TextGenerationRequest, TextGenerationResponse,
};
use crate::task::{
    AudioTranscriptionPipeline, ImageEmbeddingPipeline, ImageGenerationPipeline,
    TextGenerationPipeline,
};

#[derive(Clone)]
pub struct AutoModel {
    runtime: Runtime,
    spec: Arc<ModelSpec>,
    deployment: Arc<Mutex<Option<ModelDeployment>>>,
}

impl AutoModel {
    pub(crate) fn new(runtime: Runtime, spec: ModelSpec) -> Self {
        Self {
            runtime,
            spec: Arc::new(spec),
            deployment: Arc::new(Mutex::new(None)),
        }
    }

    pub fn spec(&self) -> &ModelSpec {
        self.spec.as_ref()
    }

    pub async fn load(&self) -> Result<ModelDeployment, CoreError> {
        self.ensure_loaded_for(self.spec.task_kind(), self.spec.dispatch.require_streaming)
            .await
    }

    pub async fn unload(&self) -> Result<(), CoreError> {
        let resolved = {
            let guard = self.deployment.lock().await;
            guard.as_ref().map(|deployment| deployment.resolved.clone())
        };

        if let Some(resolved) = resolved {
            self.runtime
                .orchestrator()
                .unload_model_backend(&resolved.backend_id)
                .await?;
            let mut guard = self.deployment.lock().await;
            *guard = None;
        }

        Ok(())
    }

    pub fn text_generation(&self) -> Result<AutoModelForTextGeneration, CoreError> {
        self.require_capability(Capability::TextGeneration)?;
        Ok(AutoModelForTextGeneration {
            model: self.clone(),
        })
    }

    pub fn audio_transcription(&self) -> Result<AutoModelForAudioTranscription, CoreError> {
        self.require_capability(Capability::AudioTranscription)?;
        Ok(AutoModelForAudioTranscription {
            model: self.clone(),
        })
    }

    pub fn image_generation(&self) -> Result<AutoModelForImageGeneration, CoreError> {
        self.require_capability(Capability::ImageGeneration)?;
        Ok(AutoModelForImageGeneration {
            model: self.clone(),
        })
    }

    pub fn image_embedding(&self) -> Result<AutoModelForImageEmbedding, CoreError> {
        self.require_capability(Capability::ImageEmbedding)?;
        Ok(AutoModelForImageEmbedding {
            model: self.clone(),
        })
    }

    pub(crate) fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    pub(crate) fn spec_arc(&self) -> Arc<ModelSpec> {
        Arc::clone(&self.spec)
    }

    pub(crate) async fn ensure_loaded_for(
        &self,
        task_kind: TaskKind,
        streaming: bool,
    ) -> Result<ModelDeployment, CoreError> {
        {
            let guard = self.deployment.lock().await;
            if let Some(existing) = guard.as_ref() {
                if streaming && !existing.resolved.supports_streaming {
                    return Err(CoreError::UnsupportedOperation {
                        backend: existing.resolved.driver_id.clone(),
                        op: "stream".to_owned(),
                    });
                }
                if existing.resolved.task_kind != task_kind {
                    return Err(CoreError::UnsupportedCapability {
                        family: format!("{:?}", existing.resolved.family),
                        capability: format!("{:?}", task_kind.capability()),
                    });
                }
                return Ok(existing.clone());
            }
        }

        let resolved = self.runtime.planner().resolve(&self.spec, task_kind, streaming)?;
        let payload = crate::task::encode_load_payload(&self.spec, &resolved)?;
        self.runtime
            .orchestrator()
            .load_model_backend(&resolved.backend_id, payload)
            .await?;

        let deployment = ModelDeployment {
            spec: self.spec.as_ref().clone(),
            resolved,
        };

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

impl std::fmt::Debug for AutoModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutoModel")
            .field("spec", &self.spec)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct AutoModelForTextGeneration {
    pub(crate) model: AutoModel,
}

impl AutoModelForTextGeneration {
    pub async fn load(&self) -> Result<ModelDeployment, CoreError> {
        self.model
            .ensure_loaded_for(
                TaskKind::TextGeneration,
                self.model.spec().dispatch.require_streaming,
            )
            .await
    }

    pub async fn unload(&self) -> Result<(), CoreError> {
        self.model.unload().await
    }

    pub async fn run(
        &self,
        request: TextGenerationRequest,
    ) -> Result<TextGenerationResponse, CoreError> {
        TextGenerationPipeline::new(self.model.clone())?
            .run(request)
            .await
    }

    pub async fn stream(
        &self,
        request: TextGenerationRequest,
    ) -> Result<
        futures::stream::BoxStream<'static, Result<TextGenerationChunk, CoreError>>,
        CoreError,
    > {
        TextGenerationPipeline::new(self.model.clone())?
            .stream(request)
            .await
    }
}

#[derive(Clone, Debug)]
pub struct AutoModelForAudioTranscription {
    pub(crate) model: AutoModel,
}

impl AutoModelForAudioTranscription {
    pub async fn load(&self) -> Result<ModelDeployment, CoreError> {
        self.model
            .ensure_loaded_for(TaskKind::AudioTranscription, false)
            .await
    }

    pub async fn unload(&self) -> Result<(), CoreError> {
        self.model.unload().await
    }

    pub async fn run(
        &self,
        request: AudioTranscriptionRequest,
    ) -> Result<AudioTranscriptionResponse, CoreError> {
        AudioTranscriptionPipeline::new(self.model.clone())?
            .run(request)
            .await
    }
}

#[derive(Clone, Debug)]
pub struct AutoModelForImageGeneration {
    pub(crate) model: AutoModel,
}

impl AutoModelForImageGeneration {
    pub async fn load(&self) -> Result<ModelDeployment, CoreError> {
        self.model
            .ensure_loaded_for(TaskKind::ImageGeneration, false)
            .await
    }

    pub async fn unload(&self) -> Result<(), CoreError> {
        self.model.unload().await
    }

    pub async fn run(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse, CoreError> {
        ImageGenerationPipeline::new(self.model.clone())?
            .run(request)
            .await
    }
}

#[derive(Clone, Debug)]
pub struct AutoModelForImageEmbedding {
    pub(crate) model: AutoModel,
}

impl AutoModelForImageEmbedding {
    pub async fn load(&self) -> Result<ModelDeployment, CoreError> {
        self.model
            .ensure_loaded_for(TaskKind::ImageEmbedding, false)
            .await
    }

    pub async fn unload(&self) -> Result<(), CoreError> {
        self.model.unload().await
    }

    pub async fn run(
        &self,
        request: ImageEmbeddingRequest,
    ) -> Result<ImageEmbeddingResponse, CoreError> {
        ImageEmbeddingPipeline::new(self.model.clone())?
            .run(request)
            .await
    }
}

impl From<AutoModelForTextGeneration> for AutoModel {
    fn from(value: AutoModelForTextGeneration) -> Self {
        value.model
    }
}

impl From<AutoModelForAudioTranscription> for AutoModel {
    fn from(value: AutoModelForAudioTranscription) -> Self {
        value.model
    }
}

impl From<AutoModelForImageGeneration> for AutoModel {
    fn from(value: AutoModelForImageGeneration) -> Self {
        value.model
    }
}

impl From<AutoModelForImageEmbedding> for AutoModel {
    fn from(value: AutoModelForImageEmbedding) -> Self {
        value.model
    }
}

impl From<(Runtime, ModelSpec)> for AutoModel {
    fn from(value: (Runtime, ModelSpec)) -> Self {
        value.0.model(value.1)
    }
}
