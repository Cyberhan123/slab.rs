use crate::application::dtos as dto;
use crate::domain::runtime::CoreError;
use crate::domain::services::{
    ExecutionHub, OnnxEmbeddingService as DomainOnnxEmbeddingService,
    OnnxTextService as DomainOnnxTextService,
};

use super::{
    LoadedService, RuntimeApplicationError, empty_slot, model_status, store_loaded, take_loaded,
};

const ONNX_TEXT_BACKEND: &str = "onnx.text";
const ONNX_EMBEDDING_BACKEND: &str = "onnx.embedding";

#[derive(Clone)]
enum SharedOnnxDeployment<TText, TEmbedding> {
    Text(TText),
    Embedding(TEmbedding),
}

type ActiveOnnxDeployment =
    SharedOnnxDeployment<DomainOnnxTextService, DomainOnnxEmbeddingService>;

impl ActiveOnnxDeployment {
    async fn unload(self) -> Result<(), CoreError> {
        match self {
            Self::Text(service) => service.unload().await,
            Self::Embedding(service) => service.unload().await,
        }
    }
}

fn clone_text_slot<TText: Clone, TEmbedding>(
    slot: &Option<SharedOnnxDeployment<TText, TEmbedding>>,
) -> Option<TText> {
    match slot.as_ref() {
        Some(SharedOnnxDeployment::Text(service)) => Some(service.clone()),
        _ => None,
    }
}

fn clone_embedding_slot<TText, TEmbedding: Clone>(
    slot: &Option<SharedOnnxDeployment<TText, TEmbedding>>,
) -> Option<TEmbedding> {
    match slot.as_ref() {
        Some(SharedOnnxDeployment::Embedding(service)) => Some(service.clone()),
        _ => None,
    }
}

fn take_text_slot<TText, TEmbedding>(
    slot: &mut Option<SharedOnnxDeployment<TText, TEmbedding>>,
) -> Option<TText> {
    match slot.take() {
        Some(SharedOnnxDeployment::Text(service)) => Some(service),
        Some(other) => {
            *slot = Some(other);
            None
        }
        None => None,
    }
}

fn take_embedding_slot<TText, TEmbedding>(
    slot: &mut Option<SharedOnnxDeployment<TText, TEmbedding>>,
) -> Option<TEmbedding> {
    match slot.take() {
        Some(SharedOnnxDeployment::Embedding(service)) => Some(service),
        Some(other) => {
            *slot = Some(other);
            None
        }
        None => None,
    }
}

#[derive(Clone)]
pub(crate) struct OnnxService {
    execution: ExecutionHub,
    active: LoadedService<ActiveOnnxDeployment>,
}

impl OnnxService {
    pub(crate) fn new(execution: ExecutionHub) -> Self {
        Self { execution, active: empty_slot() }
    }

    async fn unload_active(&self) -> Result<(), RuntimeApplicationError> {
        if let Some(previous) = take_loaded(&self.active).await {
            previous.unload().await?;
        }
        Ok(())
    }

    async fn clone_text_service(&self) -> Result<DomainOnnxTextService, RuntimeApplicationError> {
        let guard = self.active.read().await;
        clone_text_slot(&guard)
            .ok_or(CoreError::ModelNotLoaded)
            .map_err(RuntimeApplicationError::Runtime)
    }

    async fn take_text_service(&self) -> Result<DomainOnnxTextService, RuntimeApplicationError> {
        let mut guard = self.active.write().await;
        take_text_slot(&mut guard)
            .ok_or(CoreError::ModelNotLoaded)
            .map_err(RuntimeApplicationError::Runtime)
    }

    async fn clone_embedding_service(
        &self,
    ) -> Result<DomainOnnxEmbeddingService, RuntimeApplicationError> {
        let guard = self.active.read().await;
        clone_embedding_slot(&guard)
            .ok_or(CoreError::ModelNotLoaded)
            .map_err(RuntimeApplicationError::Runtime)
    }

    async fn take_embedding_service(
        &self,
    ) -> Result<DomainOnnxEmbeddingService, RuntimeApplicationError> {
        let mut guard = self.active.write().await;
        take_embedding_slot(&mut guard)
            .ok_or(CoreError::ModelNotLoaded)
            .map_err(RuntimeApplicationError::Runtime)
    }

    pub(crate) async fn load_text_model(
        &self,
        request: dto::OnnxTextLoadRequest,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        self.unload_active().await?;
        let service = DomainOnnxTextService::new(self.execution.clone(), request)?;
        service.load().await?;
        store_loaded(&self.active, SharedOnnxDeployment::Text(service)).await;
        Ok(model_status(ONNX_TEXT_BACKEND, "loaded"))
    }

    pub(crate) async fn unload_text_model(
        &self,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        let service = self.take_text_service().await?;
        service.unload().await?;
        Ok(model_status(ONNX_TEXT_BACKEND, "unloaded"))
    }

    pub(crate) async fn run_text(
        &self,
        request: dto::OnnxTextRequest,
    ) -> Result<dto::OnnxTextResponse, RuntimeApplicationError> {
        self.clone_text_service().await?.run(request).await.map_err(Into::into)
    }

    pub(crate) async fn load_embedding_model(
        &self,
        request: dto::OnnxEmbeddingLoadRequest,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        self.unload_active().await?;
        let service = DomainOnnxEmbeddingService::new(self.execution.clone(), request)?;
        service.load().await?;
        store_loaded(&self.active, SharedOnnxDeployment::Embedding(service)).await;
        Ok(model_status(ONNX_EMBEDDING_BACKEND, "loaded"))
    }

    pub(crate) async fn unload_embedding_model(
        &self,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        let service = self.take_embedding_service().await?;
        service.unload().await?;
        Ok(model_status(ONNX_EMBEDDING_BACKEND, "unloaded"))
    }

    pub(crate) async fn run_embedding(
        &self,
        request: dto::OnnxEmbeddingRequest,
    ) -> Result<dto::OnnxEmbeddingResponse, RuntimeApplicationError> {
        self.clone_embedding_service().await?.run(request).await.map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SharedOnnxDeployment, clone_embedding_slot, clone_text_slot, take_embedding_slot,
        take_text_slot,
    };

    #[test]
    fn clone_text_slot_only_reads_text_variant() {
        let text_slot = Some(SharedOnnxDeployment::<u32, u32>::Text(7));
        let embedding_slot = Some(SharedOnnxDeployment::<u32, u32>::Embedding(11));

        assert_eq!(clone_text_slot(&text_slot), Some(7));
        assert_eq!(clone_text_slot(&embedding_slot), None);
    }

    #[test]
    fn take_text_slot_preserves_embedding_variant() {
        let mut slot = Some(SharedOnnxDeployment::<u32, u32>::Embedding(9));

        assert_eq!(take_text_slot(&mut slot), None);
        assert!(matches!(slot, Some(SharedOnnxDeployment::Embedding(9))));
    }

    #[test]
    fn take_embedding_slot_extracts_matching_variant() {
        let mut slot = Some(SharedOnnxDeployment::<u32, u32>::Embedding(5));

        assert_eq!(clone_embedding_slot(&slot), Some(5));
        assert_eq!(take_embedding_slot(&mut slot), Some(5));
        assert!(slot.is_none());
    }
}
