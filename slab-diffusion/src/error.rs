use thiserror::Error;

#[derive(Error, Debug)]
pub enum DiffusionError {
    #[error("Failed to create stable diffusion context (NULL returned from new_sd_ctx)")]
    ContextCreationFailed,

    #[error("Image generation failed (NULL returned from generate_image)")]
    GenerationFailed,

    #[error("Upscaling failed (NULL data in upscaled image)")]
    UpscalerFailed,
}
