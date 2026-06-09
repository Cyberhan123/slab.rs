use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use slab_types::RuntimeDevicePreference;

use super::error::CandleDiffusionError;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffusionPipelineKind {
    StableDiffusion,
    Flux,
    StableDiffusion3,
}

impl Default for DiffusionPipelineKind {
    fn default() -> Self {
        Self::StableDiffusion
    }
}

impl std::fmt::Display for DiffusionPipelineKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::StableDiffusion => "stable_diffusion",
            Self::Flux => "flux",
            Self::StableDiffusion3 => "stable_diffusion_3",
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum StableDiffusionVersion {
    V1_5,
    V1_5Inpaint,
    V2_1,
    Sdxl,
    SdxlInpaint,
    SdxlTurbo,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FluxModelKind {
    Dev,
    Schnell,
}

impl Default for FluxModelKind {
    fn default() -> Self {
        Self::Schnell
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FluxWeightSource {
    Safetensors,
    QuantizedGguf,
}

impl Default for FluxWeightSource {
    fn default() -> Self {
        Self::Safetensors
    }
}

impl Default for StableDiffusionVersion {
    fn default() -> Self {
        Self::V2_1
    }
}

impl StableDiffusionVersion {
    pub(crate) fn is_inpaint(self) -> bool {
        matches!(self, Self::V1_5Inpaint | Self::SdxlInpaint)
    }

    pub(crate) fn latent_scale(self) -> f64 {
        match self {
            Self::SdxlTurbo => 0.13025,
            Self::V1_5 | Self::V1_5Inpaint | Self::V2_1 | Self::Sdxl | Self::SdxlInpaint => 0.18215,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandleDiffusionLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<RuntimeDevicePreference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_encoder_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_encoder2_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer2_path: Option<PathBuf>,
    #[serde(default)]
    pub pipeline: DiffusionPipelineKind,
    #[serde(default)]
    pub sd_version: StableDiffusionVersion,
    #[serde(default)]
    pub flux_model: FluxModelKind,
    #[serde(default)]
    pub flux_weight_source: FluxWeightSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flux_t5_encoder_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flux_t5_config_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flux_t5_tokenizer_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flux_clip_encoder_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flux_clip_tokenizer_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flux_autoencoder_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageGenerationRequest {
    pub prompt: String,
    #[serde(default)]
    pub negative_prompt: String,
    pub width: u32,
    pub height: u32,
    pub steps: usize,
    pub cfg_scale: f64,
    pub seed: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub init_image: Option<GeneratedImage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_image: Option<GeneratedImage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_method: Option<String>,
}

impl ImageGenerationRequest {
    pub(crate) fn validate(&self) -> Result<(), CandleDiffusionError> {
        if self.prompt.trim().is_empty() {
            return Err(CandleDiffusionError::InvalidParams {
                message: "prompt must not be empty".to_owned(),
            });
        }
        if self.width == 0 || self.height == 0 {
            return Err(CandleDiffusionError::InvalidParams {
                message: "width and height must be greater than 0".to_owned(),
            });
        }
        if !self.width.is_multiple_of(8) || !self.height.is_multiple_of(8) {
            return Err(CandleDiffusionError::InvalidParams {
                message: format!(
                    "width ({}) and height ({}) must be multiples of 8",
                    self.width, self.height
                ),
            });
        }
        if self.steps == 0 {
            return Err(CandleDiffusionError::InvalidParams {
                message: "steps must be greater than 0".to_owned(),
            });
        }
        if self.cfg_scale < 0.0 {
            return Err(CandleDiffusionError::InvalidParams {
                message: "cfg_scale must be >= 0".to_owned(),
            });
        }
        if let Some(strength) = self.strength
            && !(0.0..=1.0).contains(&strength)
        {
            return Err(CandleDiffusionError::InvalidParams {
                message: "strength must be between 0 and 1".to_owned(),
            });
        }
        if let Some(image) = self.init_image.as_ref() {
            validate_image(image, "init_image", 3)?;
        }
        if let Some(image) = self.mask_image.as_ref() {
            validate_image(image, "mask_image", 1)?;
        }
        Ok(())
    }

    pub(crate) fn validate_stable_diffusion_version(
        &self,
        version: StableDiffusionVersion,
    ) -> Result<(), CandleDiffusionError> {
        if version.is_inpaint() && (self.init_image.is_none() || self.mask_image.is_none()) {
            return Err(CandleDiffusionError::InvalidParams {
                message: "inpaint requires both init_image and mask_image".to_owned(),
            });
        }
        if !version.is_inpaint() && self.mask_image.is_some() {
            return Err(CandleDiffusionError::UnsupportedOption {
                option: "mask_image",
                message: "mask_image is only supported with inpaint Stable Diffusion versions"
                    .to_owned(),
            });
        }
        Ok(())
    }
}

impl Default for ImageGenerationRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            negative_prompt: String::new(),
            width: 512,
            height: 512,
            steps: 20,
            cfg_scale: 7.5,
            seed: 42,
            init_image: None,
            mask_image: None,
            strength: None,
            scheduler: None,
            sample_method: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeneratedImage {
    pub width: u32,
    pub height: u32,
    pub channels: u32,
    pub data: Vec<u8>,
}

fn validate_image(
    image: &GeneratedImage,
    name: &'static str,
    expected_channels: u32,
) -> Result<(), CandleDiffusionError> {
    if image.width == 0 || image.height == 0 {
        return Err(CandleDiffusionError::InvalidParams {
            message: format!("{name} width and height must be greater than 0"),
        });
    }
    if image.channels != expected_channels {
        return Err(CandleDiffusionError::InvalidParams {
            message: format!(
                "{name} must have {expected_channels} channel(s), got {}",
                image.channels
            ),
        });
    }
    let expected_len = image.width as usize * image.height as usize * image.channels as usize;
    if image.data.len() != expected_len {
        return Err(CandleDiffusionError::InvalidParams {
            message: format!("{name} data length is {}, expected {expected_len}", image.data.len()),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_dimensions_are_rejected() {
        let request = ImageGenerationRequest {
            prompt: "test".to_owned(),
            width: 513,
            ..ImageGenerationRequest::default()
        };
        assert!(matches!(request.validate(), Err(CandleDiffusionError::InvalidParams { .. })));
    }

    #[test]
    fn sdxl_turbo_uses_turbo_latent_scale() {
        assert_eq!(StableDiffusionVersion::SdxlTurbo.latent_scale(), 0.13025);
    }

    #[test]
    fn invalid_strength_is_rejected() {
        let request = ImageGenerationRequest {
            prompt: "test".to_owned(),
            strength: Some(1.5),
            ..ImageGenerationRequest::default()
        };
        assert!(matches!(request.validate(), Err(CandleDiffusionError::InvalidParams { .. })));
    }

    #[test]
    fn invalid_init_image_buffer_is_rejected() {
        let request = ImageGenerationRequest {
            prompt: "test".to_owned(),
            init_image: Some(GeneratedImage {
                width: 2,
                height: 2,
                channels: 3,
                data: vec![0; 11],
            }),
            ..ImageGenerationRequest::default()
        };
        assert!(matches!(request.validate(), Err(CandleDiffusionError::InvalidParams { .. })));
    }

    #[test]
    fn inpaint_requires_init_and_mask() {
        let request = ImageGenerationRequest {
            prompt: "test".to_owned(),
            init_image: Some(GeneratedImage {
                width: 8,
                height: 8,
                channels: 3,
                data: vec![0; 8 * 8 * 3],
            }),
            ..ImageGenerationRequest::default()
        };
        let error = request
            .validate_stable_diffusion_version(StableDiffusionVersion::V1_5Inpaint)
            .expect_err("inpaint should require a mask");
        assert!(matches!(error, CandleDiffusionError::InvalidParams { .. }));
    }

    #[test]
    fn mask_requires_inpaint_version() {
        let request = ImageGenerationRequest {
            prompt: "test".to_owned(),
            mask_image: Some(GeneratedImage {
                width: 8,
                height: 8,
                channels: 1,
                data: vec![0; 8 * 8],
            }),
            ..ImageGenerationRequest::default()
        };
        let error = request
            .validate_stable_diffusion_version(StableDiffusionVersion::V1_5)
            .expect_err("mask should require an inpaint version");
        assert!(matches!(
            error,
            CandleDiffusionError::UnsupportedOption { option: "mask_image", .. }
        ));
    }

    #[test]
    fn flux_defaults_match_fast_local_layout() {
        let config = CandleDiffusionLoadConfig {
            model_path: PathBuf::from("flux.safetensors"),
            vae_path: None,
            device: None,
            text_encoder_path: None,
            text_encoder2_path: None,
            tokenizer_path: None,
            tokenizer2_path: None,
            pipeline: DiffusionPipelineKind::Flux,
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
        assert_eq!(config.flux_model, FluxModelKind::Schnell);
        assert_eq!(config.flux_weight_source, FluxWeightSource::Safetensors);
    }
}
