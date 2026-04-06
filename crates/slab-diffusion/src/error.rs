use thiserror::Error;

#[derive(Error, Debug)]
pub enum DiffusionError {
    #[error("Failed to create stable diffusion context (NULL returned from new_sd_ctx)")]
    ContextCreationFailed,

    #[error("Failed to get stable diffusion context")]
    ContextNull,

    #[error("Image generation failed (NULL returned from generate_image)")]
    GenerationFailed,

    #[error("Invalid diffusion parameters: {0}")]
    InvalidParameters(String),

    /// Reserved for future upscaling support (ESRGAN / RealESRGAN).
    #[error("Upscaling failed (NULL data in upscaled image)")]
    UpscalerFailed,

    #[error("Backend list is unavailable")]
    BackendListUnavailable,
}
