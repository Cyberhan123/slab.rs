use crate::application::dtos as dto;
use crate::domain::runtime::CoreError;
use crate::domain::services::{ExecutionHub, GgmlParakeetService as DomainGgmlParakeetService};

use super::{
    LoadedService, RuntimeApplicationError, clone_loaded, empty_slot, model_status, store_loaded,
    take_loaded,
};

#[derive(Clone)]
pub(crate) struct GgmlParakeetService {
    execution: ExecutionHub,
    loaded: LoadedService<DomainGgmlParakeetService>,
}

impl GgmlParakeetService {
    pub(crate) fn new(execution: ExecutionHub) -> Self {
        Self { execution, loaded: empty_slot() }
    }

    pub(crate) async fn load_model(
        &self,
        request: dto::GgmlParakeetLoadRequest,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        if let Some(previous) = take_loaded(&self.loaded).await {
            previous.unload().await?;
        }

        let service = DomainGgmlParakeetService::new(self.execution.clone(), request)?;
        service.load().await?;
        store_loaded(&self.loaded, service).await;
        Ok(model_status("ggml.parakeet", "loaded"))
    }

    pub(crate) async fn unload_model(&self) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        let service = take_loaded(&self.loaded)
            .await
            .ok_or(CoreError::ModelNotLoaded)
            .map_err(RuntimeApplicationError::Runtime)?;
        service.unload().await?;
        Ok(model_status("ggml.parakeet", "unloaded"))
    }

    pub(crate) async fn transcribe(
        &self,
        request: dto::GgmlParakeetTranscribeRequest,
    ) -> Result<dto::GgmlParakeetTranscribeResponse, RuntimeApplicationError> {
        clone_loaded(&self.loaded).await?.transcribe(request).await.map_err(Into::into)
    }
}
