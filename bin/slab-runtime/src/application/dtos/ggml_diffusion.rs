use slab_proto::slab::ipc::v1 as pb;

use super::{
    GgmlDiffusionGenerateImageRequest, GgmlDiffusionGenerateImageResponse,
    GgmlDiffusionGenerateVideoRequest, GgmlDiffusionGenerateVideoResponse,
    GgmlDiffusionLoadRequest, ProtoConversionError, decode_optional_path, decode_raw_image,
    encode_raw_image,
};

pub(crate) fn decode_ggml_diffusion_load_request(
    request: &pb::GgmlDiffusionLoadRequest,
) -> Result<GgmlDiffusionLoadRequest, ProtoConversionError> {
    Ok(GgmlDiffusionLoadRequest {
        model_path: decode_optional_path(request.model_path.as_ref()),
        diffusion_model_path: decode_optional_path(request.diffusion_model_path.as_ref()),
        vae_path: decode_optional_path(request.vae_path.as_ref()),
        taesd_path: decode_optional_path(request.taesd_path.as_ref()),
        clip_l_path: decode_optional_path(request.clip_l_path.as_ref()),
        clip_g_path: decode_optional_path(request.clip_g_path.as_ref()),
        t5xxl_path: decode_optional_path(request.t5xxl_path.as_ref()),
        clip_vision_path: decode_optional_path(request.clip_vision_path.as_ref()),
        control_net_path: decode_optional_path(request.control_net_path.as_ref()),
        flash_attn: request.flash_attn,
        vae_device: request.vae_device.clone(),
        clip_device: request.clip_device.clone(),
        offload_params_to_cpu: request.offload_params_to_cpu,
        enable_mmap: request.enable_mmap,
        n_threads: request.n_threads,
    })
}

pub(crate) fn decode_ggml_diffusion_generate_image_request(
    request: &pb::GgmlDiffusionGenerateImageRequest,
) -> Result<GgmlDiffusionGenerateImageRequest, ProtoConversionError> {
    Ok(GgmlDiffusionGenerateImageRequest {
        prompt: request.prompt.clone(),
        negative_prompt: request.negative_prompt.clone(),
        width: request.width,
        height: request.height,
        init_image: request.init_image.as_ref().map(decode_raw_image),
        count: request.count,
        cfg_scale: request.cfg_scale,
        guidance: request.guidance,
        sample_steps: request.sample_steps,
        seed: request.seed,
        sample_method: request.sample_method.clone(),
        scheduler: request.scheduler.clone(),
        clip_skip: request.clip_skip,
        strength: request.strength,
        eta: request.eta,
    })
}

pub(crate) fn encode_ggml_diffusion_generate_image_response(
    response: &GgmlDiffusionGenerateImageResponse,
) -> pb::GgmlDiffusionGenerateImageResponse {
    pb::GgmlDiffusionGenerateImageResponse {
        images: response.images.iter().map(encode_raw_image).collect(),
    }
}

pub(crate) fn decode_ggml_diffusion_generate_video_request(
    request: &pb::GgmlDiffusionGenerateVideoRequest,
) -> Result<GgmlDiffusionGenerateVideoRequest, ProtoConversionError> {
    Ok(GgmlDiffusionGenerateVideoRequest {
        prompt: request.prompt.clone(),
        negative_prompt: request.negative_prompt.clone(),
        width: request.width,
        height: request.height,
        init_image: request.init_image.as_ref().map(decode_raw_image),
        video_frames: request.video_frames,
        fps: request.fps,
        cfg_scale: request.cfg_scale,
        guidance: request.guidance,
        sample_steps: request.sample_steps,
        seed: request.seed,
        sample_method: request.sample_method.clone(),
        scheduler: request.scheduler.clone(),
        strength: request.strength,
    })
}

pub(crate) fn encode_ggml_diffusion_generate_video_response(
    response: &GgmlDiffusionGenerateVideoResponse,
) -> pb::GgmlDiffusionGenerateVideoResponse {
    pb::GgmlDiffusionGenerateVideoResponse {
        frames: response.frames.iter().map(encode_raw_image).collect(),
    }
}
