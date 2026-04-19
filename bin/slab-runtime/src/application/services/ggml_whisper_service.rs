use crate::application::dtos as dto;
use crate::domain::runtime::CoreError;
use crate::domain::services::{ExecutionHub, GgmlWhisperService as DomainGgmlWhisperService};

use super::{
    LoadedService, RuntimeApplicationError, clone_loaded, empty_slot, model_status, store_loaded,
    take_loaded,
};

#[derive(Clone)]
pub(crate) struct GgmlWhisperService {
    execution: ExecutionHub,
    loaded: LoadedService<DomainGgmlWhisperService>,
}

impl GgmlWhisperService {
    pub(crate) fn new(execution: ExecutionHub) -> Self {
        Self { execution, loaded: empty_slot() }
    }

    pub(crate) async fn load_model(
        &self,
        request: dto::GgmlWhisperLoadRequest,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        if let Some(previous) = take_loaded(&self.loaded).await {
            previous.unload().await?;
        }

        let service = DomainGgmlWhisperService::new(self.execution.clone(), request)?;
        service.load().await?;
        store_loaded(&self.loaded, service).await;
        Ok(model_status("ggml.whisper", "loaded"))
    }

    pub(crate) async fn unload_model(&self) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        let service = take_loaded(&self.loaded)
            .await
            .ok_or(CoreError::ModelNotLoaded)
            .map_err(RuntimeApplicationError::Runtime)?;
        service.unload().await?;
        Ok(model_status("ggml.whisper", "unloaded"))
    }

    pub(crate) async fn transcribe(
        &self,
        request: dto::GgmlWhisperTranscribeRequest,
    ) -> Result<dto::GgmlWhisperTranscribeResponse, RuntimeApplicationError> {
        clone_loaded(&self.loaded).await?.transcribe(request).await.map_err(Into::into)
    }
}
