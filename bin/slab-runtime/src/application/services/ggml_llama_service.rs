use futures::stream::BoxStream;
use slab_runtime_core::CoreError;

use crate::application::dtos as dto;
use crate::domain::services::{ExecutionHub, GgmlLlamaService as DomainGgmlLlamaService};

use super::{
    LoadedService, RuntimeApplicationError, clone_loaded, empty_slot, model_status, store_loaded,
    take_loaded,
};

#[derive(Clone)]
pub(crate) struct GgmlLlamaService {
    execution: ExecutionHub,
    loaded: LoadedService<DomainGgmlLlamaService>,
}

impl GgmlLlamaService {
    pub(crate) fn new(execution: ExecutionHub) -> Self {
        Self { execution, loaded: empty_slot() }
    }

    pub(crate) async fn load_model(
        &self,
        request: dto::GgmlLlamaLoadRequest,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        if let Some(previous) = take_loaded(&self.loaded).await {
            previous.unload().await?;
        }

        let service = DomainGgmlLlamaService::new(self.execution.clone(), request)?;
        service.load().await?;
        store_loaded(&self.loaded, service).await;
        Ok(model_status("ggml.llama", "loaded"))
    }

    pub(crate) async fn unload_model(&self) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        let service = take_loaded(&self.loaded)
            .await
            .ok_or(CoreError::ModelNotLoaded)
            .map_err(RuntimeApplicationError::Runtime)?;
        service.unload().await?;
        Ok(model_status("ggml.llama", "unloaded"))
    }

    pub(crate) async fn chat(
        &self,
        request: dto::GgmlLlamaChatRequest,
    ) -> Result<dto::LlamaChatResponse, RuntimeApplicationError> {
        clone_loaded(&self.loaded).await?.chat(request).await.map_err(Into::into)
    }

    pub(crate) async fn chat_stream(
        &self,
        request: dto::GgmlLlamaChatRequest,
    ) -> Result<
        BoxStream<'static, Result<dto::LlamaChatStreamChunk, slab_runtime_core::CoreError>>,
        RuntimeApplicationError,
    > {
        clone_loaded(&self.loaded).await?.chat_stream(request).await.map_err(Into::into)
    }
}
