use futures::stream::BoxStream;
use slab_runtime_core::CoreError;

use crate::application::dtos as dto;
use crate::domain::services::{
    CandleLlamaService as DomainCandleLlamaService,
    CandleWhisperService as DomainCandleWhisperService, ExecutionHub,
};

use super::{
    LoadedService, RuntimeApplicationError, clone_loaded, empty_slot, model_status, store_loaded,
    take_loaded,
};

#[derive(Clone)]
pub(crate) struct CandleTransformersService {
    execution: ExecutionHub,
    llama: LoadedService<DomainCandleLlamaService>,
    whisper: LoadedService<DomainCandleWhisperService>,
}

impl CandleTransformersService {
    pub(crate) fn new(execution: ExecutionHub) -> Self {
        Self { execution, llama: empty_slot(), whisper: empty_slot() }
    }

    pub(crate) async fn load_llama_model(
        &self,
        request: dto::CandleLlamaLoadRequest,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        if let Some(previous) = take_loaded(&self.llama).await {
            previous.unload().await?;
        }

        let service = DomainCandleLlamaService::new(self.execution.clone(), request)?;
        service.load().await?;
        store_loaded(&self.llama, service).await;
        Ok(model_status("candle.llama", "loaded"))
    }

    pub(crate) async fn unload_llama_model(
        &self,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        let service = take_loaded(&self.llama)
            .await
            .ok_or(CoreError::ModelNotLoaded)
            .map_err(RuntimeApplicationError::Runtime)?;
        service.unload().await?;
        Ok(model_status("candle.llama", "unloaded"))
    }

    pub(crate) async fn chat(
        &self,
        request: dto::CandleChatRequest,
    ) -> Result<dto::LlamaChatResponse, RuntimeApplicationError> {
        clone_loaded(&self.llama).await?.chat(request).await.map_err(Into::into)
    }

    pub(crate) async fn chat_stream(
        &self,
        request: dto::CandleChatRequest,
    ) -> Result<
        BoxStream<'static, Result<dto::LlamaChatStreamChunk, slab_runtime_core::CoreError>>,
        RuntimeApplicationError,
    > {
        clone_loaded(&self.llama).await?.chat_stream(request).await.map_err(Into::into)
    }

    pub(crate) async fn load_whisper_model(
        &self,
        request: dto::CandleWhisperLoadRequest,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        if let Some(previous) = take_loaded(&self.whisper).await {
            previous.unload().await?;
        }

        let service = DomainCandleWhisperService::new(self.execution.clone(), request)?;
        service.load().await?;
        store_loaded(&self.whisper, service).await;
        Ok(model_status("candle.whisper", "loaded"))
    }

    pub(crate) async fn unload_whisper_model(
        &self,
    ) -> Result<dto::ModelStatus, RuntimeApplicationError> {
        let service = take_loaded(&self.whisper)
            .await
            .ok_or(CoreError::ModelNotLoaded)
            .map_err(RuntimeApplicationError::Runtime)?;
        service.unload().await?;
        Ok(model_status("candle.whisper", "unloaded"))
    }

    pub(crate) async fn transcribe(
        &self,
        request: dto::CandleWhisperTranscribeRequest,
    ) -> Result<dto::CandleWhisperTranscribeResponse, RuntimeApplicationError> {
        clone_loaded(&self.whisper).await?.transcribe(request).await.map_err(Into::into)
    }
}
