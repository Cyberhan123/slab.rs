use slab_diffusion::{
    GuidanceParams as DiffusionGuidanceParams, SampleParams as DiffusionSampleParams, SlgParams,
};
use slab_runtime_core::backend::RequestRoute;

use crate::application::dtos as dto;
use crate::domain::models::{
    CandleDiffusionLoadConfig, ImageGenerationRequest, ImageGenerationResponse,
};
use crate::domain::runtime::CoreError;

use super::ExecutionHub;
use super::driver_runtime::DriverRuntime;
use super::helpers::{contract_image_to_raw_image, invalid_model, required_path, required_string};

#[derive(Clone, Debug)]
pub(crate) struct CandleDiffusionService {
    runtime: DriverRuntime,
}

impl CandleDiffusionService {
    pub(crate) fn new(
        execution: ExecutionHub,
        request: dto::CandleDiffusionLoadRequest,
    ) -> Result<Self, CoreError> {
        let model_path = required_path("candle_diffusion.model_path", request.model_path)?;
        let sd_version = required_string("candle_diffusion.sd_version", request.sd_version)?;
        let load_payload = CandleDiffusionLoadConfig {
            model_path: model_path.clone(),
            vae_path: request.vae_path,
            sd_version,
        };

        Ok(Self {
            runtime: DriverRuntime::new_typed(
                execution,
                "candle.diffusion",
                "candle.diffusion",
                load_payload,
            ),
        })
    }

    pub(crate) async fn load(&self) -> Result<(), CoreError> {
        self.runtime.load().await
    }

    pub(crate) async fn unload(&self) -> Result<(), CoreError> {
        self.runtime.unload().await
    }

    pub(crate) async fn generate_image(
        &self,
        request: dto::CandleDiffusionGenerateImageRequest,
    ) -> Result<dto::CandleDiffusionGenerateImageResponse, CoreError> {
        let response: ImageGenerationResponse = self
            .runtime
            .invoke_without_options(
                RequestRoute::InferenceImage,
                build_image_request(request)?,
                Vec::new(),
            )
            .await?;

        Ok(dto::CandleDiffusionGenerateImageResponse {
            images: response.images.iter().map(contract_image_to_raw_image).collect(),
        })
    }
}

fn build_image_request(
    request: dto::CandleDiffusionGenerateImageRequest,
) -> Result<ImageGenerationRequest, CoreError> {
    if request.width.is_some_and(|value| value == 0) {
        return Err(invalid_model("candle_diffusion.width", "must be >= 1"));
    }
    if request.height.is_some_and(|value| value == 0) {
        return Err(invalid_model("candle_diffusion.height", "must be >= 1"));
    }
    if request.batch_count.is_some_and(|value| value == 0) {
        return Err(invalid_model("candle_diffusion.batch_count", "must be >= 1"));
    }
    if request.sample_steps.is_some_and(|value| value < 1) {
        return Err(invalid_model("candle_diffusion.sample_steps", "must be >= 1"));
    }

    let mut sample_params = DiffusionSampleParams::default();
    if let Some(guidance_scale) = request.guidance_scale {
        sample_params.guidance = Some(DiffusionGuidanceParams {
            txt_cfg: guidance_scale,
            img_cfg: guidance_scale,
            distilled_guidance: guidance_scale,
            slg: SlgParams::default(),
        });
    }
    sample_params.sample_steps = request.sample_steps;

    Ok(ImageGenerationRequest {
        prompt: required_string("candle_diffusion.prompt", request.prompt)?,
        negative_prompt: request.negative_prompt,
        width: request.width,
        height: request.height,
        sample_steps: sample_params.sample_steps.and_then(|value| u32::try_from(value).ok()),
        guidance_scale: sample_params.guidance.map(|guidance| guidance.txt_cfg),
        seed: request
            .seed
            .map(|value| {
                u64::try_from(value)
                    .map_err(|_| invalid_model("candle_diffusion.seed", "must be >= 0"))
            })
            .transpose()?,
        batch_count: request.batch_count.unwrap_or(1),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::build_image_request;
    use crate::application::dtos::CandleDiffusionGenerateImageRequest;

    #[test]
    fn build_image_request_normalizes_seed_and_steps() {
        let request = build_image_request(CandleDiffusionGenerateImageRequest {
            prompt: Some("cat".to_owned()),
            negative_prompt: None,
            width: Some(512),
            height: Some(512),
            batch_count: Some(3),
            sample_steps: Some(24),
            guidance_scale: Some(7.0),
            seed: Some(11),
        })
        .expect("request should map");

        assert_eq!(request.prompt, "cat");
        assert_eq!(request.sample_steps, Some(24));
        assert_eq!(request.guidance_scale, Some(7.0));
        assert_eq!(request.seed, Some(11));
        assert_eq!(request.batch_count, 3);
    }
}
