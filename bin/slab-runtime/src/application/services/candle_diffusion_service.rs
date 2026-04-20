use crate::application::dtos as dto;
use crate::domain::runtime::CoreError;
use crate::domain::services::{
    CandleDiffusionService as DomainCandleDiffusionService, ExecutionHub,
};

use super::{
    LoadedService, RuntimeApplicationError, clone_loaded, empty_slot, model_status, store_loaded,
    take_loaded,
};

#[derive(Clone)]
pub(crate) struct CandleDiffusionService {
    execution: ExecutionHub,
    diffusion: LoadedService<DomainCandleDiffusionService>,
}

impl CandleDiffusionService {
    pub(crate) fn new(execution: ExecutionHub) -> Self {
        Self { execution, diffusion: empty_slot() }
    }

    pub(crate) async fn load_model(
        &self,
        request: dto::CandleDiffusionLoadRequest,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        if let Some(previous) = take_loaded(&self.diffusion).await {
            previous.unload().await?;
        }

        let service = DomainCandleDiffusionService::new(self.execution.clone(), request)?;
        service.load().await?;
        store_loaded(&self.diffusion, service).await;
        Ok(model_status("candle.diffusion", "loaded"))
    }

    pub(crate) async fn unload_model(&self) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        let service = take_loaded(&self.diffusion)
            .await
            .ok_or(CoreError::ModelNotLoaded)
            .map_err(RuntimeApplicationError::Runtime)?;
        service.unload().await?;
        Ok(model_status("candle.diffusion", "unloaded"))
    }

    pub(crate) async fn generate_image(
        &self,
        request: dto::CandleDiffusionGenerateImageRequest,
    ) -> Result<dto::CandleDiffusionGenerateImageResponse, RuntimeApplicationError> {
        clone_loaded(&self.diffusion).await?.generate_image(request).await.map_err(Into::into)
    }
}
