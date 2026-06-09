use super::config::{
    CandleDiffusionLoadConfig, DiffusionPipelineKind, GeneratedImage, ImageGenerationRequest,
};
use super::error::CandleDiffusionError;
use super::flux::FluxPipeline;
use super::stable::StableDiffusionPipeline;
use crate::device::resolve_device;
use crate::runtime::CandleRuntimeEngine;

enum LoadedPipeline {
    StableDiffusion(Box<StableDiffusionPipeline>),
    Flux(Box<FluxPipeline>),
}

pub struct CandleDiffusionEngine {
    pipeline: Option<LoadedPipeline>,
}

impl CandleDiffusionEngine {
    pub fn new() -> Self {
        Self { pipeline: None }
    }
}

impl Default for CandleDiffusionEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CandleRuntimeEngine for CandleDiffusionEngine {
    type Error = CandleDiffusionError;
    type InferenceRequest = ImageGenerationRequest;
    type InferenceResponse = GeneratedImage;
    type LoadConfig = CandleDiffusionLoadConfig;

    fn load_model(&mut self, config: Self::LoadConfig) -> Result<(), Self::Error> {
        let device = resolve_device(config.device).map_err(|error| {
            CandleDiffusionError::load_model(config.model_path.display(), error)
        })?;
        let pipeline = match config.pipeline {
            DiffusionPipelineKind::StableDiffusion => LoadedPipeline::StableDiffusion(Box::new(
                StableDiffusionPipeline::load(config, device)?,
            )),
            DiffusionPipelineKind::Flux => {
                LoadedPipeline::Flux(Box::new(FluxPipeline::load(config, device)?))
            }
            DiffusionPipelineKind::StableDiffusion3 => {
                return Err(CandleDiffusionError::UnsupportedModel {
                    kind: config.pipeline.to_string(),
                    message: "candle-transformers 0.10.2 exposes mmdit building blocks but not a complete Stable Diffusion 3 pipeline in this crate".to_owned(),
                });
            }
        };
        self.pipeline = Some(pipeline);
        Ok(())
    }

    fn unload_model(&mut self) {
        self.pipeline = None;
    }

    fn is_model_loaded(&self) -> bool {
        self.pipeline.is_some()
    }

    fn infer(
        &mut self,
        request: Self::InferenceRequest,
    ) -> Result<Self::InferenceResponse, Self::Error> {
        request.validate()?;
        match self.pipeline.as_mut().ok_or(CandleDiffusionError::ModelNotLoaded)? {
            LoadedPipeline::StableDiffusion(pipeline) => pipeline.generate(&request),
            LoadedPipeline::Flux(pipeline) => pipeline.generate(&request),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diffusion::{FluxModelKind, FluxWeightSource, StableDiffusionVersion};

    #[test]
    fn new_engine_is_unloaded() {
        assert!(!CandleDiffusionEngine::new().is_model_loaded());
    }

    #[test]
    fn stable_diffusion_3_is_explicitly_unsupported() {
        let mut engine = CandleDiffusionEngine::new();
        let config = CandleDiffusionLoadConfig {
            model_path: "missing.safetensors".into(),
            vae_path: None,
            device: None,
            text_encoder_path: None,
            text_encoder2_path: None,
            tokenizer_path: None,
            tokenizer2_path: None,
            pipeline: DiffusionPipelineKind::StableDiffusion3,
            sd_version: StableDiffusionVersion::default(),
            flux_model: FluxModelKind::default(),
            flux_weight_source: FluxWeightSource::default(),
            flux_t5_encoder_path: None,
            flux_t5_config_path: None,
            flux_t5_tokenizer_path: None,
            flux_clip_encoder_path: None,
            flux_clip_tokenizer_path: None,
            flux_autoencoder_path: None,
        };
        let error =
            engine.load_model(config).expect_err("SD3 should fail before loading local assets");
        assert!(matches!(error, CandleDiffusionError::UnsupportedModel { .. }));
    }
}
