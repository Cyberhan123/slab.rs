use std::path::{Path, PathBuf};

use candle_core::{D, DType, Device, IndexOp, Tensor};
use candle_transformers::models::stable_diffusion;
use tokenizers::Tokenizer;

use super::config::{
    CandleDiffusionLoadConfig, GeneratedImage, ImageGenerationRequest, StableDiffusionVersion,
};
use super::error::CandleDiffusionError;

pub(crate) struct StableDiffusionPipeline {
    version: StableDiffusionVersion,
    sd_config: stable_diffusion::StableDiffusionConfig,
    unet: stable_diffusion::unet_2d::UNet2DConditionModel,
    vae: stable_diffusion::vae::AutoEncoderKL,
    clip: stable_diffusion::clip::ClipTextTransformer,
    clip2: Option<stable_diffusion::clip::ClipTextTransformer>,
    tokenizer: Tokenizer,
    tokenizer2: Option<Tokenizer>,
    device: Device,
}

impl StableDiffusionPipeline {
    pub(crate) fn load(
        config: CandleDiffusionLoadConfig,
        device: Device,
    ) -> Result<Self, CandleDiffusionError> {
        let dtype = DType::F32;
        let root = model_root(&config.model_path);
        let sd_config = sd_config(config.sd_version, None, None)?;
        let tokenizer_path =
            config.tokenizer_path.unwrap_or_else(|| root.join("tokenizer").join("tokenizer.json"));
        let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|error| {
            CandleDiffusionError::LoadModel {
                model_path: tokenizer_path.display().to_string(),
                message: format!("failed to load tokenizer: {error}"),
            }
        })?;
        let text_encoder_path = config
            .text_encoder_path
            .unwrap_or_else(|| root.join("text_encoder").join("model.safetensors"));
        let clip = stable_diffusion::build_clip_transformer(
            &sd_config.clip,
            &text_encoder_path,
            &device,
            dtype,
        )
        .map_err(|error| CandleDiffusionError::load_model(text_encoder_path.display(), error))?;

        let (tokenizer2, clip2) = if let Some(clip2_config) = sd_config.clip2.as_ref() {
            let tokenizer2_path = config
                .tokenizer2_path
                .unwrap_or_else(|| root.join("tokenizer_2").join("tokenizer.json"));
            let tokenizer2 = Tokenizer::from_file(&tokenizer2_path).map_err(|error| {
                CandleDiffusionError::LoadModel {
                    model_path: tokenizer2_path.display().to_string(),
                    message: format!("failed to load tokenizer_2: {error}"),
                }
            })?;
            let text_encoder2_path = config
                .text_encoder2_path
                .unwrap_or_else(|| root.join("text_encoder_2").join("model.safetensors"));
            let clip2 = stable_diffusion::build_clip_transformer(
                clip2_config,
                &text_encoder2_path,
                &device,
                dtype,
            )
            .map_err(|error| {
                CandleDiffusionError::load_model(text_encoder2_path.display(), error)
            })?;
            (Some(tokenizer2), Some(clip2))
        } else {
            (None, None)
        };

        let vae_path = config
            .vae_path
            .unwrap_or_else(|| root.join("vae").join("diffusion_pytorch_model.safetensors"));
        let vae = sd_config
            .build_vae(&vae_path, &device, dtype)
            .map_err(|error| CandleDiffusionError::load_model(vae_path.display(), error))?;
        let in_channels = if config.sd_version.is_inpaint() { 9 } else { 4 };
        let unet =
            sd_config.build_unet(&config.model_path, &device, in_channels, false, dtype).map_err(
                |error| CandleDiffusionError::load_model(config.model_path.display(), error),
            )?;

        Ok(Self {
            version: config.sd_version,
            sd_config,
            unet,
            vae,
            clip,
            clip2,
            tokenizer,
            tokenizer2,
            device,
        })
    }

    pub(crate) fn generate(
        &self,
        request: &ImageGenerationRequest,
    ) -> Result<GeneratedImage, CandleDiffusionError> {
        request.validate_stable_diffusion_version(self.version)?;
        if request.scheduler.is_some() || request.sample_method.is_some() {
            return Err(CandleDiffusionError::UnsupportedOption {
                option: "scheduler",
                message:
                    "Candle stable_diffusion uses the scheduler baked into StableDiffusionConfig"
                        .to_owned(),
            });
        }

        let device = &self.device;
        let use_guidance = request.cfg_scale > 1.0;
        let text_embeddings = self.text_embeddings(
            request.prompt.trim(),
            &request.negative_prompt,
            use_guidance,
            device,
        )?;
        let mut scheduler = self
            .sd_config
            .build_scheduler(request.steps)
            .map_err(|error| CandleDiffusionError::inference(format!("scheduler: {error}")))?;
        device
            .set_seed(request.seed)
            .map_err(|error| CandleDiffusionError::inference(format!("seed RNG: {error}")))?;
        let latent_h = (request.height / 8) as usize;
        let latent_w = (request.width / 8) as usize;
        let init_image = request
            .init_image
            .as_ref()
            .map(|image| {
                generated_image_to_tensor(image, request.width, request.height, device, DType::F32)
            })
            .transpose()?;
        let init_latent_dist =
            init_image.as_ref().map(|image| self.vae.encode(image)).transpose().map_err(
                |error| CandleDiffusionError::inference(format!("VAE encode init image: {error}")),
            )?;
        let timesteps = scheduler.timesteps().to_vec();
        let t_start = if init_latent_dist.is_some() {
            let strength = request.strength.unwrap_or(0.8) as f64;
            request.steps.saturating_sub((request.steps as f64 * strength) as usize)
        } else {
            0
        };
        let (mask_latents, mask) = self.inpainting_tensors(
            request,
            init_image.as_ref(),
            use_guidance,
            device,
            DType::F32,
        )?;
        let mut latents = match init_latent_dist {
            Some(init_latent_dist) => {
                let latents = (init_latent_dist.sample().map_err(|error| {
                    CandleDiffusionError::inference(format!("sample init latents: {error}"))
                })? * self.version.latent_scale())
                .map_err(|error| {
                    CandleDiffusionError::inference(format!("scale init latents: {error}"))
                })?;
                if t_start < timesteps.len() {
                    let noise = latents.randn_like(0f64, 1f64).map_err(|error| {
                        CandleDiffusionError::inference(format!("init latent noise: {error}"))
                    })?;
                    scheduler.add_noise(&latents, noise, timesteps[t_start]).map_err(|error| {
                        CandleDiffusionError::inference(format!("add init noise: {error}"))
                    })?
                } else {
                    latents
                }
            }
            None => {
                let latents =
                    Tensor::randn(0.0f32, 1.0f32, (1usize, 4, latent_h, latent_w), device)
                        .map_err(|error| {
                            CandleDiffusionError::inference(format!("noise tensor: {error}"))
                        })?;
                (latents * scheduler.init_noise_sigma()).map_err(|error| {
                    CandleDiffusionError::inference(format!("scale noise: {error}"))
                })?
            }
        };

        for (timestep_index, timestep) in timesteps.iter().copied().enumerate() {
            if timestep_index < t_start {
                continue;
            }
            let latent_model_input = if use_guidance {
                Tensor::cat(&[&latents, &latents], 0)
            } else {
                Ok(latents.clone())
            }
            .and_then(|tensor| scheduler.scale_model_input(tensor, timestep))
            .map_err(|error| {
                CandleDiffusionError::inference(format!("scale model input: {error}"))
            })?;
            let latent_model_input = if self.version.is_inpaint() {
                Tensor::cat(
                    &[
                        &latent_model_input,
                        mask.as_ref().ok_or_else(|| CandleDiffusionError::InvalidParams {
                            message: "inpaint mask missing".to_owned(),
                        })?,
                        mask_latents.as_ref().ok_or_else(|| {
                            CandleDiffusionError::InvalidParams {
                                message: "inpaint mask latents missing".to_owned(),
                            }
                        })?,
                    ],
                    1,
                )
                .map_err(|error| {
                    CandleDiffusionError::inference(format!("inpaint latent concat: {error}"))
                })?
            } else {
                latent_model_input
            };
            let noise_pred =
                self.unet.forward(&latent_model_input, timestep as f64, &text_embeddings).map_err(
                    |error| CandleDiffusionError::inference(format!("UNet forward: {error}")),
                )?;
            let noise_pred = if use_guidance {
                let noise_uncond = noise_pred.i(..1).map_err(|error| {
                    CandleDiffusionError::inference(format!("slice uncond: {error}"))
                })?;
                let noise_text = noise_pred.i(1..).map_err(|error| {
                    CandleDiffusionError::inference(format!("slice text: {error}"))
                })?;
                (&noise_text - &noise_uncond)
                    .and_then(|delta| delta * request.cfg_scale)
                    .and_then(|delta| &noise_uncond + delta)
                    .map_err(|error| {
                        CandleDiffusionError::inference(format!("guidance: {error}"))
                    })?
            } else {
                noise_pred
            };
            latents = scheduler.step(&noise_pred, timestep, &latents).map_err(|error| {
                CandleDiffusionError::inference(format!("scheduler step: {error}"))
            })?;
        }

        self.decode_latents(latents)
    }

    fn text_embeddings(
        &self,
        prompt: &str,
        negative_prompt: &str,
        use_guidance: bool,
        device: &Device,
    ) -> Result<Tensor, CandleDiffusionError> {
        let mut embeddings = Vec::new();
        embeddings.push(encode_text(
            &self.tokenizer,
            &self.clip,
            &self.sd_config.clip,
            prompt,
            negative_prompt,
            use_guidance,
            device,
        )?);
        if let (Some(tokenizer2), Some(clip2), Some(clip2_config)) =
            (&self.tokenizer2, &self.clip2, self.sd_config.clip2.as_ref())
        {
            embeddings.push(encode_text(
                tokenizer2,
                clip2,
                clip2_config,
                prompt,
                negative_prompt,
                use_guidance,
                device,
            )?);
        }
        Tensor::cat(&embeddings, D::Minus1)
            .map_err(|error| CandleDiffusionError::inference(format!("embedding concat: {error}")))
    }

    fn decode_latents(&self, latents: Tensor) -> Result<GeneratedImage, CandleDiffusionError> {
        let scaled = (latents / self.version.latent_scale())
            .map_err(|error| CandleDiffusionError::inference(format!("latent scale: {error}")))?;
        let decoded = self
            .vae
            .decode(&scaled)
            .map_err(|error| CandleDiffusionError::inference(format!("VAE decode: {error}")))?;
        let image = ((decoded / 2.0)
            .and_then(|tensor| tensor + 0.5f64)
            .and_then(|tensor| tensor.clamp(0.0, 1.0))
            .and_then(|tensor| tensor * 255.0)
            .and_then(|tensor| tensor.to_dtype(DType::U8)))
        .map_err(|error| CandleDiffusionError::inference(format!("image tensor: {error}")))?;
        let image = if image.dims().len() == 4 { image.squeeze(0) } else { Ok(image) }.map_err(
            |error| CandleDiffusionError::inference(format!("image batch squeeze: {error}")),
        )?;
        let (channels, height, width) = image
            .dims3()
            .map_err(|error| CandleDiffusionError::inference(format!("image dims: {error}")))?;
        let data = image
            .permute((1, 2, 0))
            .and_then(|tensor| tensor.flatten_all())
            .and_then(|tensor| tensor.to_vec1::<u8>())
            .map_err(|error| CandleDiffusionError::inference(format!("image data: {error}")))?;

        Ok(GeneratedImage {
            width: width as u32,
            height: height as u32,
            channels: channels as u32,
            data,
        })
    }

    fn inpainting_tensors(
        &self,
        request: &ImageGenerationRequest,
        init_image: Option<&Tensor>,
        use_guidance: bool,
        device: &Device,
        dtype: DType,
    ) -> Result<(Option<Tensor>, Option<Tensor>), CandleDiffusionError> {
        if !self.version.is_inpaint() {
            return Ok((None, None));
        }
        let mask_image = request.mask_image.as_ref().ok_or_else(|| {
            CandleDiffusionError::InvalidParams { message: "inpaint mask_image missing".to_owned() }
        })?;
        let image = init_image.ok_or_else(|| CandleDiffusionError::InvalidParams {
            message: "inpaint init_image missing".to_owned(),
        })?;
        let mask = mask_image_to_tensor(mask_image, request.width, request.height, device, dtype)?;
        let xmask = mask
            .le(0.5)
            .and_then(|tensor| tensor.repeat(&[1, 3, 1, 1]))
            .and_then(|tensor| tensor.to_dtype(dtype))
            .map_err(|error| {
                CandleDiffusionError::inference(format!("inpaint image mask: {error}"))
            })?;
        let masked_img = (image * xmask)
            .map_err(|error| CandleDiffusionError::inference(format!("masked image: {error}")))?;
        let shape = masked_img.shape();
        let height = shape.dims()[2] / 8;
        let width = shape.dims()[3] / 8;
        let mask = mask
            .interpolate2d(width, height)
            .map_err(|error| CandleDiffusionError::inference(format!("resize mask: {error}")))?;
        let mask_latents =
            (self.vae.encode(&masked_img).and_then(|dist| dist.sample()).map_err(|error| {
                CandleDiffusionError::inference(format!("masked image latents: {error}"))
            })? * self.version.latent_scale())
            .map_err(|error| {
                CandleDiffusionError::inference(format!("scale mask latents: {error}"))
            })?;
        if use_guidance {
            let mask_latents =
                Tensor::cat(&[&mask_latents, &mask_latents], 0).map_err(|error| {
                    CandleDiffusionError::inference(format!("mask latent guidance concat: {error}"))
                })?;
            let mask = Tensor::cat(&[&mask, &mask], 0).map_err(|error| {
                CandleDiffusionError::inference(format!("mask guidance concat: {error}"))
            })?;
            Ok((Some(mask_latents), Some(mask)))
        } else {
            Ok((Some(mask_latents), Some(mask)))
        }
    }
}

fn generated_image_to_tensor(
    image: &GeneratedImage,
    expected_width: u32,
    expected_height: u32,
    device: &Device,
    dtype: DType,
) -> Result<Tensor, CandleDiffusionError> {
    if image.width != expected_width || image.height != expected_height {
        return Err(CandleDiffusionError::InvalidParams {
            message: format!(
                "init_image dimensions {}x{} must match request dimensions {expected_width}x{expected_height}",
                image.width, image.height
            ),
        });
    }
    Tensor::from_vec(
        image.data.clone(),
        (image.height as usize, image.width as usize, image.channels as usize),
        device,
    )
    .and_then(|tensor| tensor.permute((2, 0, 1)))
    .and_then(|tensor| tensor.to_dtype(dtype))
    .and_then(|tensor| tensor.affine(2.0 / 255.0, -1.0))
    .and_then(|tensor| tensor.unsqueeze(0))
    .map_err(|error| CandleDiffusionError::inference(format!("init image tensor: {error}")))
}

fn mask_image_to_tensor(
    image: &GeneratedImage,
    expected_width: u32,
    expected_height: u32,
    device: &Device,
    dtype: DType,
) -> Result<Tensor, CandleDiffusionError> {
    if image.width != expected_width || image.height != expected_height {
        return Err(CandleDiffusionError::InvalidParams {
            message: format!(
                "mask_image dimensions {}x{} must match request dimensions {expected_width}x{expected_height}",
                image.width, image.height
            ),
        });
    }
    Tensor::from_vec(
        image.data.clone(),
        (1usize, image.height as usize, image.width as usize),
        device,
    )
    .and_then(|tensor| tensor.to_dtype(dtype))
    .and_then(|tensor| tensor.affine(1.0 / 255.0, 0.0))
    .and_then(|tensor| tensor.unsqueeze(0))
    .map_err(|error| CandleDiffusionError::inference(format!("mask image tensor: {error}")))
}

fn encode_text(
    tokenizer: &Tokenizer,
    clip: &stable_diffusion::clip::ClipTextTransformer,
    clip_config: &stable_diffusion::clip::Config,
    prompt: &str,
    negative_prompt: &str,
    use_guidance: bool,
    device: &Device,
) -> Result<Tensor, CandleDiffusionError> {
    let prompt_ids = tokenize_and_pad(tokenizer, clip_config, prompt)?;
    let prompt_tokens =
        Tensor::new(prompt_ids.as_slice(), device).and_then(|tensor| tensor.unsqueeze(0)).map_err(
            |error| CandleDiffusionError::inference(format!("prompt token tensor: {error}")),
        )?;
    let prompt_embeddings =
        clip.forward_with_mask(&prompt_tokens, usize::MAX).map_err(|error| {
            CandleDiffusionError::inference(format!("CLIP prompt forward: {error}"))
        })?;
    if !use_guidance {
        return Ok(prompt_embeddings);
    }
    let negative_ids = tokenize_and_pad(tokenizer, clip_config, negative_prompt)?;
    let negative_tokens = Tensor::new(negative_ids.as_slice(), device)
        .and_then(|tensor| tensor.unsqueeze(0))
        .map_err(|error| {
            CandleDiffusionError::inference(format!("negative token tensor: {error}"))
        })?;
    let negative_embeddings =
        clip.forward_with_mask(&negative_tokens, usize::MAX).map_err(|error| {
            CandleDiffusionError::inference(format!("CLIP negative forward: {error}"))
        })?;
    Tensor::cat(&[negative_embeddings, prompt_embeddings], 0).map_err(|error| {
        CandleDiffusionError::inference(format!("guidance embedding concat: {error}"))
    })
}

fn tokenize_and_pad(
    tokenizer: &Tokenizer,
    clip_config: &stable_diffusion::clip::Config,
    text: &str,
) -> Result<Vec<u32>, CandleDiffusionError> {
    let pad_token = clip_config.pad_with.as_deref().unwrap_or("<|endoftext|>");
    let pad_id = tokenizer.get_vocab(true).get(pad_token).copied().ok_or_else(|| {
        CandleDiffusionError::InvalidAssetLayout {
            path: "tokenizer.json".to_owned(),
            message: format!("missing CLIP pad token {pad_token}"),
        }
    })?;
    let mut tokens = tokenizer
        .encode(text, true)
        .map_err(|error| CandleDiffusionError::inference(format!("tokenize prompt: {error}")))?
        .get_ids()
        .to_vec();
    if tokens.len() > clip_config.max_position_embeddings {
        return Err(CandleDiffusionError::InvalidParams {
            message: format!(
                "prompt has {} tokens, max is {}",
                tokens.len(),
                clip_config.max_position_embeddings
            ),
        });
    }
    while tokens.len() < clip_config.max_position_embeddings {
        tokens.push(pad_id);
    }
    Ok(tokens)
}

fn sd_config(
    version: StableDiffusionVersion,
    height: Option<usize>,
    width: Option<usize>,
) -> Result<stable_diffusion::StableDiffusionConfig, CandleDiffusionError> {
    Ok(match version {
        StableDiffusionVersion::V1_5 | StableDiffusionVersion::V1_5Inpaint => {
            stable_diffusion::StableDiffusionConfig::v1_5(None, height, width)
        }
        StableDiffusionVersion::V2_1 => {
            stable_diffusion::StableDiffusionConfig::v2_1(None, height, width)
        }
        StableDiffusionVersion::Sdxl | StableDiffusionVersion::SdxlInpaint => {
            stable_diffusion::StableDiffusionConfig::sdxl(None, height, width)
        }
        StableDiffusionVersion::SdxlTurbo => {
            stable_diffusion::StableDiffusionConfig::sdxl_turbo(None, height, width)
        }
    })
}

fn model_root(model_path: &Path) -> PathBuf {
    if model_path.is_dir() {
        return model_path.to_path_buf();
    }
    let parent = model_path.parent().unwrap_or_else(|| Path::new("."));
    if parent.file_name().and_then(|value| value.to_str()) == Some("unet") {
        parent.parent().unwrap_or(parent).to_path_buf()
    } else {
        parent.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_root_uses_parent_of_unet_dir() {
        let root = model_root(Path::new("model/unet/diffusion_pytorch_model.safetensors"));
        assert_eq!(root, PathBuf::from("model"));
    }
}
