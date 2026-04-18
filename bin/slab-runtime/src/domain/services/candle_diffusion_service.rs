use slab_diffusion::{
    GuidanceParams as DiffusionGuidanceParams, ImgParams as DiffusionImgParams,
    SampleParams as DiffusionSampleParams, SlgParams,
};
use slab_runtime_core::{CoreError, Payload};
use slab_types::{CandleDiffusionLoadConfig, Capability, ModelFamily};

use slab_proto::convert::dto;

use super::ExecutionHub;
use super::driver_runtime::DriverRuntime;
use super::helpers::{
    decode_images_payload, invalid_model, model_spec, required_path, required_string,
};

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
        let load_payload = Payload::typed(CandleDiffusionLoadConfig {
            model_path: model_path.clone(),
            vae_path: request.vae_path,
            sd_version,
        });

        Ok(Self {
            runtime: DriverRuntime::new(
                execution,
                model_spec(ModelFamily::Diffusion, Capability::ImageGeneration, model_path),
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
        let payload = self
            .runtime
            .submit(
                Capability::ImageGeneration,
                false,
                Payload::typed(build_image_params(request)?),
                Vec::new(),
                Payload::None,
            )
            .await?
            .result()
            .await?;

        Ok(dto::CandleDiffusionGenerateImageResponse {
            images: decode_images_payload(payload, "candle_diffusion")?,
        })
    }
}

fn build_image_params(
    request: dto::CandleDiffusionGenerateImageRequest,
) -> Result<DiffusionImgParams, CoreError> {
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

    Ok(DiffusionImgParams {
        prompt: Some(required_string("candle_diffusion.prompt", request.prompt)?),
        negative_prompt: request.negative_prompt,
        width: request.width,
        height: request.height,
        sample_params: (sample_params != DiffusionSampleParams::default()).then_some(sample_params),
        seed: request.seed,
        batch_count: request.batch_count,
        ..Default::default()
    })
}
