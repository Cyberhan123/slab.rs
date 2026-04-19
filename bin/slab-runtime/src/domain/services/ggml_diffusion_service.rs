use std::str::FromStr;

use slab_diffusion::{
    ContextParams as DiffusionContextParams, GuidanceParams as DiffusionGuidanceParams,
    ImgParams as DiffusionImgParams, SampleMethod as DiffusionSampleMethod,
    SampleParams as DiffusionSampleParams, Scheduler as DiffusionScheduler, SlgParams,
};
use slab_runtime_core::{CoreError, Payload};
use slab_types::{Capability, ModelFamily};

use crate::application::dtos as dto;

use super::ExecutionHub;
use super::driver_runtime::DriverRuntime;
use super::helpers::{
    decode_images_payload, invalid_model, model_spec, raw_image_to_diffusion_image, required_path,
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
        let load_payload = Payload::typed(DiffusionContextParams {
            model_path: Some(model_path.clone()),
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
            ..Default::default()
        });

        Ok(Self {
            runtime: DriverRuntime::new(
                execution,
                model_spec(ModelFamily::Diffusion, Capability::ImageGeneration, model_path),
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
        Ok(dto::GgmlDiffusionGenerateImageResponse {
            images: decode_images_payload(payload, "ggml_diffusion_image")?,
        })
    }

    pub(crate) async fn generate_video(
        &self,
        request: dto::GgmlDiffusionGenerateVideoRequest,
    ) -> Result<dto::GgmlDiffusionGenerateVideoResponse, CoreError> {
        let payload = self
            .runtime
            .submit(
                Capability::ImageGeneration,
                false,
                Payload::typed(build_video_as_image_params(request)?),
                Vec::new(),
                Payload::None,
            )
            .await?
            .result()
            .await?;
        Ok(dto::GgmlDiffusionGenerateVideoResponse {
            frames: decode_images_payload(payload, "ggml_diffusion_video")?,
        })
    }
}

fn build_image_params(
    request: dto::GgmlDiffusionGenerateImageRequest,
) -> Result<DiffusionImgParams, CoreError> {
    let prompt = required_string("ggml_diffusion.prompt", request.prompt)?;
    let width = request
        .width
        .ok_or_else(|| invalid_model("ggml_diffusion.width", "missing required value"))?;
    let height = request
        .height
        .ok_or_else(|| invalid_model("ggml_diffusion.height", "missing required value"))?;

    let sample_method = request
        .sample_method
        .as_deref()
        .map(DiffusionSampleMethod::from_str)
        .transpose()
        .map_err(|error| invalid_model("ggml_diffusion.sample_method", error))?;
    let scheduler = request
        .scheduler
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

    Ok(DiffusionImgParams {
        prompt: Some(prompt),
        negative_prompt: request.negative_prompt,
        clip_skip: request.clip_skip,
        init_image: request
            .init_image
            .as_ref()
            .map(|image| raw_image_to_diffusion_image(image, "ggml_diffusion_image"))
            .transpose()?,
        width: Some(width),
        height: Some(height),
        sample_params: (sample_params != DiffusionSampleParams::default()).then_some(sample_params),
        strength: request.strength,
        seed: request.seed,
        batch_count: Some(request.count.unwrap_or(1)),
        ..Default::default()
    })
}

fn build_video_as_image_params(
    request: dto::GgmlDiffusionGenerateVideoRequest,
) -> Result<DiffusionImgParams, CoreError> {
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
        .as_deref()
        .map(DiffusionSampleMethod::from_str)
        .transpose()
        .map_err(|error| invalid_model("ggml_diffusion.sample_method", error))?;
    let scheduler = request
        .scheduler
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

    Ok(DiffusionImgParams {
        prompt: Some(prompt),
        negative_prompt: request.negative_prompt,
        init_image: request
            .init_image
            .as_ref()
            .map(|image| raw_image_to_diffusion_image(image, "ggml_diffusion_video"))
            .transpose()?,
        width: Some(width),
        height: Some(height),
        sample_params: (sample_params != DiffusionSampleParams::default()).then_some(sample_params),
        strength: request.strength,
        seed: request.seed,
        batch_count: Some(video_frames),
        ..Default::default()
    })
}
