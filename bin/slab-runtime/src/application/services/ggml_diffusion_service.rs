use slab_runtime_core::CoreError;

use slab_proto::convert::dto;

use crate::domain::services::{ExecutionHub, GgmlDiffusionService as DomainGgmlDiffusionService};

use super::{
    LoadedService, RuntimeApplicationError, clone_loaded, empty_slot, model_status, store_loaded,
    take_loaded,
};

#[derive(Clone)]
pub(crate) struct GgmlDiffusionService {
    execution: ExecutionHub,
    loaded: LoadedService<DomainGgmlDiffusionService>,
}

impl GgmlDiffusionService {
    pub(crate) fn new(execution: ExecutionHub) -> Self {
        Self { execution, loaded: empty_slot() }
    }

    pub(crate) async fn load_model(
        &self,
        request: dto::GgmlDiffusionLoadRequest,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        if let Some(previous) = take_loaded(&self.loaded).await {
            previous.unload().await?;
        }

        let service = DomainGgmlDiffusionService::new(self.execution.clone(), request)?;
        service.load().await?;
        store_loaded(&self.loaded, service).await;
        Ok(model_status("ggml.diffusion", "loaded"))
    }

    pub(crate) async fn unload_model(&self) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        let service = take_loaded(&self.loaded)
            .await
            .ok_or(CoreError::ModelNotLoaded)
            .map_err(RuntimeApplicationError::Runtime)?;
        service.unload().await?;
        Ok(model_status("ggml.diffusion", "unloaded"))
    }

    pub(crate) async fn generate_image(
        &self,
        request: dto::GgmlDiffusionGenerateImageRequest,
    ) -> Result<dto::GgmlDiffusionGenerateImageResponse, RuntimeApplicationError> {
        clone_loaded(&self.loaded).await?.generate_image(request).await.map_err(Into::into)
    }

    pub(crate) async fn generate_video(
        &self,
        request: dto::GgmlDiffusionGenerateVideoRequest,
    ) -> Result<dto::GgmlDiffusionGenerateVideoResponse, RuntimeApplicationError> {
        clone_loaded(&self.loaded).await?.generate_video(request).await.map_err(Into::into)
    }
}
