use std::str::FromStr;

use slab_diffusion::{
    GuidanceParams as DiffusionGuidanceParams, SampleMethod as DiffusionSampleMethod,
    SampleParams as DiffusionSampleParams, Scheduler as DiffusionScheduler, SlgParams,
};
use slab_runtime_core::backend::RequestRoute;

use crate::application::dtos as dto;
use crate::domain::models::{
    GgmlDiffusionLoadConfig, ImageGenerationRequest, ImageGenerationResponse,
};
use crate::domain::runtime::CoreError;

use super::ExecutionHub;
use super::driver_runtime::DriverRuntime;
use super::helpers::{
    contract_image_to_raw_image, invalid_model, raw_image_to_generated_image, required_path,
    required_string,
};

#[derive(Clone, Debug)]
pub(crate) struct GgmlDiffusionService {
    runtime: DriverRuntime,
}

impl GgmlDiffusionService {
    pub(crate) fn new(
        execution: ExecutionHub,
        request: dto::GgmlDiffusionLoadRequest,
    ) -> Result<Self, CoreError> {
        let model_path = required_path("ggml_diffusion.model_path", request.model_path)?;
        let load_payload = GgmlDiffusionLoadConfig {
            model_path: model_path.clone(),
            diffusion_model_path: request.diffusion_model_path,
            vae_path: request.vae_path,
            taesd_path: request.taesd_path,
            clip_l_path: request.clip_l_path,
            clip_g_path: request.clip_g_path,
            t5xxl_path: request.t5xxl_path,
            clip_vision_path: request.clip_vision_path,
            control_net_path: request.control_net_path,
            flash_attn: request.flash_attn,
            vae_device: request.vae_device,
            clip_device: request.clip_device,
            offload_params_to_cpu: request.offload_params_to_cpu,
            enable_mmap: request.enable_mmap,
            n_threads: request.n_threads,
        };

        Ok(Self {
            runtime: DriverRuntime::new_typed(
                execution,
                "ggml.diffusion",
                "ggml.diffusion",
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
        request: dto::GgmlDiffusionGenerateImageRequest,
    ) -> Result<dto::GgmlDiffusionGenerateImageResponse, CoreError> {
        let response: ImageGenerationResponse = self
            .runtime
            .invoke_without_options(
                RequestRoute::InferenceImage,
                build_image_request(request)?,
                Vec::new(),
            )
            .await?;
        Ok(dto::GgmlDiffusionGenerateImageResponse {
            images: response.images.iter().map(contract_image_to_raw_image).collect(),
        })
    }

    pub(crate) async fn generate_video(
        &self,
        request: dto::GgmlDiffusionGenerateVideoRequest,
    ) -> Result<dto::GgmlDiffusionGenerateVideoResponse, CoreError> {
        let response: ImageGenerationResponse = self
            .runtime
            .invoke_without_options(
                RequestRoute::InferenceImage,
                build_video_as_image_request(request)?,
                Vec::new(),
            )
            .await?;
        Ok(dto::GgmlDiffusionGenerateVideoResponse {
            frames: response.images.iter().map(contract_image_to_raw_image).collect(),
        })
    }
}

fn build_image_request(
    request: dto::GgmlDiffusionGenerateImageRequest,
) -> Result<ImageGenerationRequest, CoreError> {
    let prompt = required_string("ggml_diffusion.prompt", request.prompt)?;
    let width = request
        .width
        .ok_or_else(|| invalid_model("ggml_diffusion.width", "missing required value"))?;
    let height = request
        .height
        .ok_or_else(|| invalid_model("ggml_diffusion.height", "missing required value"))?;

    let sample_method = request
        .sample_method
        .clone()
        .as_deref()
        .map(DiffusionSampleMethod::from_str)
        .transpose()
        .map_err(|error| invalid_model("ggml_diffusion.sample_method", error))?;
    let scheduler = request
        .scheduler
        .clone()
        .as_deref()
        .map(DiffusionScheduler::from_str)
        .transpose()
        .map_err(|error| invalid_model("ggml_diffusion.scheduler", error))?;

    if width == 0 {
        return Err(invalid_model("ggml_diffusion.width", "must be >= 1"));
    }
    if height == 0 {
        return Err(invalid_model("ggml_diffusion.height", "must be >= 1"));
    }
    if let Some(count) = request.count
        && count == 0
    {
        return Err(invalid_model("ggml_diffusion.count", "must be >= 1"));
    }
    if let Some(steps) = request.sample_steps
        && steps < 1
    {
        return Err(invalid_model("ggml_diffusion.sample_steps", "must be >= 1"));
    }

    let mut sample_params = DiffusionSampleParams::default();
    if request.cfg_scale.is_some() || request.guidance.is_some() {
        let cfg_scale = request.cfg_scale.or(request.guidance).unwrap_or_default();
        let distilled_guidance = request.guidance.or(request.cfg_scale).unwrap_or_default();
        sample_params.guidance = Some(DiffusionGuidanceParams {
            txt_cfg: cfg_scale,
            img_cfg: cfg_scale,
            distilled_guidance,
            slg: SlgParams::default(),
        });
    }
    sample_params.sample_method = sample_method;
    sample_params.scheduler = scheduler;
    sample_params.sample_steps = request.sample_steps;
    sample_params.eta = request.eta;

    Ok(ImageGenerationRequest {
        prompt,
        negative_prompt: request.negative_prompt,
        clip_skip: request.clip_skip,
        init_image: request
            .init_image
            .as_ref()
            .map(|image| raw_image_to_generated_image(image, "ggml_diffusion_image"))
            .transpose()?,
        width: Some(width),
        height: Some(height),
        sample_method: request.sample_method,
        scheduler: request.scheduler,
        sample_steps: sample_params.sample_steps.and_then(|steps| u32::try_from(steps).ok()),
        eta: sample_params.eta,
        guidance_scale: sample_params.guidance.as_ref().map(|guidance| guidance.txt_cfg),
        distilled_guidance: sample_params
            .guidance
            .as_ref()
            .map(|guidance| guidance.distilled_guidance),
        strength: request.strength,
        seed: request
            .seed
            .map(|value| {
                u64::try_from(value)
                    .map_err(|_| invalid_model("ggml_diffusion.seed", "must be >= 0"))
            })
            .transpose()?,
        batch_count: request.count.unwrap_or(1),
    })
}

fn build_video_as_image_request(
    request: dto::GgmlDiffusionGenerateVideoRequest,
) -> Result<ImageGenerationRequest, CoreError> {
    if request.fps.is_some() {
        return Err(invalid_model(
            "ggml_diffusion.fps",
            "video fps is not representable by the current ggml runtime payload",
        ));
    }

    let prompt = required_string("ggml_diffusion.prompt", request.prompt)?;
    let width = request
        .width
        .ok_or_else(|| invalid_model("ggml_diffusion.width", "missing required value"))?;
    let height = request
        .height
        .ok_or_else(|| invalid_model("ggml_diffusion.height", "missing required value"))?;
    let video_frames = request.video_frames.unwrap_or(16);

    let sample_method = request
        .sample_method
        .clone()
        .as_deref()
        .map(DiffusionSampleMethod::from_str)
        .transpose()
        .map_err(|error| invalid_model("ggml_diffusion.sample_method", error))?;
    let scheduler = request
        .scheduler
        .clone()
        .as_deref()
        .map(DiffusionScheduler::from_str)
        .transpose()
        .map_err(|error| invalid_model("ggml_diffusion.scheduler", error))?;

    if width == 0 {
        return Err(invalid_model("ggml_diffusion.width", "must be >= 1"));
    }
    if height == 0 {
        return Err(invalid_model("ggml_diffusion.height", "must be >= 1"));
    }
    if video_frames == 0 {
        return Err(invalid_model("ggml_diffusion.video_frames", "must be >= 1"));
    }
    if let Some(steps) = request.sample_steps
        && steps < 1
    {
        return Err(invalid_model("ggml_diffusion.sample_steps", "must be >= 1"));
    }

    let mut sample_params = DiffusionSampleParams::default();
    if request.cfg_scale.is_some() || request.guidance.is_some() {
        let cfg_scale = request.cfg_scale.or(request.guidance).unwrap_or_default();
        let distilled_guidance = request.guidance.or(request.cfg_scale).unwrap_or_default();
        sample_params.guidance = Some(DiffusionGuidanceParams {
            txt_cfg: cfg_scale,
            img_cfg: cfg_scale,
            distilled_guidance,
            slg: SlgParams::default(),
        });
    }
    sample_params.sample_method = sample_method;
    sample_params.scheduler = scheduler;
    sample_params.sample_steps = request.sample_steps;

    Ok(ImageGenerationRequest {
        prompt,
        negative_prompt: request.negative_prompt,
        clip_skip: None,
        init_image: request
            .init_image
            .as_ref()
            .map(|image| raw_image_to_generated_image(image, "ggml_diffusion_video"))
            .transpose()?,
        width: Some(width),
        height: Some(height),
        sample_method: request.sample_method,
        scheduler: request.scheduler,
        sample_steps: sample_params.sample_steps.and_then(|steps| u32::try_from(steps).ok()),
        eta: None,
        guidance_scale: sample_params.guidance.as_ref().map(|guidance| guidance.txt_cfg),
        distilled_guidance: sample_params
            .guidance
            .as_ref()
            .map(|guidance| guidance.distilled_guidance),
        strength: request.strength,
        seed: request
            .seed
            .map(|value| {
                u64::try_from(value)
                    .map_err(|_| invalid_model("ggml_diffusion.seed", "must be >= 0"))
            })
            .transpose()?,
        batch_count: video_frames,
    })
}

#[cfg(test)]
mod tests {
    use super::{build_image_request, build_video_as_image_request};
    use crate::application::dtos::{
        GgmlDiffusionGenerateImageRequest, GgmlDiffusionGenerateVideoRequest,
    };

    #[test]
    fn build_image_request_preserves_validated_runtime_contract_fields() {
        let request = build_image_request(GgmlDiffusionGenerateImageRequest {
            prompt: Some("cat".to_owned()),
            width: Some(512),
            height: Some(512),
            count: Some(2),
            sample_steps: Some(30),
            seed: Some(7),
            sample_method: Some("euler".to_owned()),
            scheduler: Some("karras".to_owned()),
            clip_skip: Some(1),
            eta: Some(0.2),
            guidance: Some(6.5),
            strength: Some(0.8),
            ..Default::default()
        })
        .expect("image request should map");

        assert_eq!(request.prompt, "cat");
        assert_eq!(request.sample_method.as_deref(), Some("euler"));
        assert_eq!(request.scheduler.as_deref(), Some("karras"));
        assert_eq!(request.sample_steps, Some(30));
        assert_eq!(request.seed, Some(7));
        assert_eq!(request.clip_skip, Some(1));
        assert_eq!(request.eta, Some(0.2));
        assert_eq!(request.batch_count, 2);
    }

    #[test]
    fn build_video_request_sets_shared_runtime_defaults() {
        let request = build_video_as_image_request(GgmlDiffusionGenerateVideoRequest {
            prompt: Some("cat".to_owned()),
            width: Some(640),
            height: Some(480),
            video_frames: Some(16),
            sample_steps: Some(20),
            seed: Some(9),
            sample_method: Some("euler".to_owned()),
            scheduler: Some("karras".to_owned()),
            ..Default::default()
        })
        .expect("video request should map");

        assert_eq!(request.sample_method.as_deref(), Some("euler"));
        assert_eq!(request.scheduler.as_deref(), Some("karras"));
        assert_eq!(request.sample_steps, Some(20));
        assert_eq!(request.seed, Some(9));
        assert_eq!(request.clip_skip, None);
        assert_eq!(request.eta, None);
        assert_eq!(request.batch_count, 16);
    }
}
