use slab_runtime_core::CoreError;

use slab_proto::convert::dto;

use crate::domain::services::{
    ExecutionHub, OnnxEmbeddingService as DomainOnnxEmbeddingService,
    OnnxTextService as DomainOnnxTextService,
};

use super::{
    LoadedService, RuntimeApplicationError, clone_loaded, empty_slot, model_status, store_loaded,
    take_loaded,
};

#[derive(Clone)]
pub(crate) struct OnnxService {
    execution: ExecutionHub,
    text: LoadedService<DomainOnnxTextService>,
    embedding: LoadedService<DomainOnnxEmbeddingService>,
}

impl OnnxService {
    pub(crate) fn new(execution: ExecutionHub) -> Self {
        Self { execution, text: empty_slot(), embedding: empty_slot() }
    }

    pub(crate) async fn load_text_model(
        &self,
        request: dto::OnnxTextLoadRequest,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        if let Some(previous) = take_loaded(&self.text).await {
            previous.unload().await?;
        }

        let service = DomainOnnxTextService::new(self.execution.clone(), request)?;
        service.load().await?;
        store_loaded(&self.text, service).await;
        Ok(model_status("onnx.text", "loaded"))
    }

    pub(crate) async fn unload_text_model(
        &self,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        let service = take_loaded(&self.text)
            .await
            .ok_or(CoreError::ModelNotLoaded)
            .map_err(RuntimeApplicationError::Runtime)?;
        service.unload().await?;
        Ok(model_status("onnx.text", "unloaded"))
    }

    pub(crate) async fn run_text(
        &self,
        request: dto::OnnxTextRequest,
    ) -> Result<dto::OnnxTextResponse, RuntimeApplicationError> {
        clone_loaded(&self.text).await?.run(request).await.map_err(Into::into)
    }

    pub(crate) async fn load_embedding_model(
        &self,
        request: dto::OnnxEmbeddingLoadRequest,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        if let Some(previous) = take_loaded(&self.embedding).await {
            previous.unload().await?;
        }

        let service = DomainOnnxEmbeddingService::new(self.execution.clone(), request)?;
        service.load().await?;
        store_loaded(&self.embedding, service).await;
        Ok(model_status("onnx.embedding", "loaded"))
    }

    pub(crate) async fn unload_embedding_model(
        &self,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        let service = take_loaded(&self.embedding)
            .await
            .ok_or(CoreError::ModelNotLoaded)
            .map_err(RuntimeApplicationError::Runtime)?;
        service.unload().await?;
        Ok(model_status("onnx.embedding", "unloaded"))
    }

    pub(crate) async fn run_embedding(
        &self,
        request: dto::OnnxEmbeddingRequest,
    ) -> Result<dto::OnnxEmbeddingResponse, RuntimeApplicationError> {
        clone_loaded(&self.embedding).await?.run(request).await.map_err(Into::into)
    }
}
