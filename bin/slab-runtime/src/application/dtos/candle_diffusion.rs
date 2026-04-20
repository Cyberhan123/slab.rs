use slab_proto::slab::ipc::v1 as pb;

use super::{
    CandleDiffusionGenerateImageRequest, CandleDiffusionGenerateImageResponse,
    CandleDiffusionLoadRequest, ProtoConversionError, decode_optional_path, encode_raw_image,
};

pub(crate) fn decode_candle_diffusion_load_request(
    request: &pb::CandleDiffusionLoadRequest,
) -> Result<CandleDiffusionLoadRequest, ProtoConversionError> {
    Ok(CandleDiffusionLoadRequest {
        model_path: decode_optional_path(request.model_path.as_ref()),
        vae_path: decode_optional_path(request.vae_path.as_ref()),
        sd_version: request.sd_version.clone(),
    })
}

pub(crate) fn decode_candle_diffusion_generate_image_request(
    request: &pb::CandleDiffusionGenerateImageRequest,
) -> Result<CandleDiffusionGenerateImageRequest, ProtoConversionError> {
    Ok(CandleDiffusionGenerateImageRequest {
        prompt: request.prompt.clone(),
        negative_prompt: request.negative_prompt.clone(),
        width: request.width,
        height: request.height,
        batch_count: request.batch_count,
        sample_steps: request.sample_steps,
        guidance_scale: request.guidance_scale,
        seed: request.seed,
    })
}

pub(crate) fn encode_candle_diffusion_generate_image_response(
    response: &CandleDiffusionGenerateImageResponse,
) -> pb::CandleDiffusionGenerateImageResponse {
    pb::CandleDiffusionGenerateImageResponse {
        images: response.images.iter().map(encode_raw_image).collect(),
    }
}
