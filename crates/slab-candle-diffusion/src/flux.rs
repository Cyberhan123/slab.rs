use std::path::{Path, PathBuf};

use candle_core::{DType, Device, Module, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::{clip, flux, t5};
use tokenizers::Tokenizer;

use crate::config::{
    CandleDiffusionLoadConfig, FluxModelKind, FluxWeightSource, GeneratedImage,
    ImageGenerationRequest,
};
use crate::error::CandleDiffusionError;

enum FluxTransformer {
    Normal(flux::model::Flux),
    Quantized(flux::quantized_model::Flux),
}

pub(crate) struct FluxPipeline {
    model_kind: FluxModelKind,
    transformer: FluxTransformer,
    autoencoder: flux::autoencoder::AutoEncoder,
    t5: t5::T5EncoderModel,
    t5_tokenizer: Tokenizer,
    clip: clip::text_model::ClipTextTransformer,
    clip_tokenizer: Tokenizer,
}

impl FluxPipeline {
    pub(crate) fn load(config: CandleDiffusionLoadConfig) -> Result<Self, CandleDiffusionError> {
        let device = Device::Cpu;
        let dtype = DType::F32;
        let root = model_root(&config.model_path);
        let transformer_path = flux_transformer_path(&config, &root);
        let t5_path = required_path(
            config
                .flux_t5_encoder_path
                .unwrap_or_else(|| root.join("t5").join("model.safetensors")),
            "flux_t5_encoder_path",
        )?;
        let t5_config_path = required_path(
            config.flux_t5_config_path.unwrap_or_else(|| root.join("t5").join("config.json")),
            "flux_t5_config_path",
        )?;
        let t5_tokenizer_path = required_path(
            config.flux_t5_tokenizer_path.unwrap_or_else(|| root.join("t5").join("tokenizer.json")),
            "flux_t5_tokenizer_path",
        )?;
        let clip_path = required_path(
            config
                .flux_clip_encoder_path
                .unwrap_or_else(|| root.join("clip").join("model.safetensors")),
            "flux_clip_encoder_path",
        )?;
        let clip_tokenizer_path = required_path(
            config
                .flux_clip_tokenizer_path
                .unwrap_or_else(|| root.join("clip").join("tokenizer.json")),
            "flux_clip_tokenizer_path",
        )?;
        let autoencoder_path = required_path(
            config.flux_autoencoder_path.unwrap_or_else(|| root.join("ae.safetensors")),
            "flux_autoencoder_path",
        )?;

        let model_cfg = match config.flux_model {
            FluxModelKind::Dev => flux::model::Config::dev(),
            FluxModelKind::Schnell => flux::model::Config::schnell(),
        };
        let transformer = match config.flux_weight_source {
            FluxWeightSource::Safetensors => {
                let vb = unsafe {
                    VarBuilder::from_mmaped_safetensors(
                        &[transformer_path.as_path()],
                        dtype,
                        &device,
                    )
                    .map_err(|error| {
                        CandleDiffusionError::load_model(transformer_path.display(), error)
                    })?
                };
                flux::model::Flux::new(&model_cfg, vb).map(FluxTransformer::Normal).map_err(
                    |error| CandleDiffusionError::load_model(transformer_path.display(), error),
                )?
            }
            FluxWeightSource::QuantizedGguf => {
                let vb = candle_transformers::quantized_var_builder::VarBuilder::from_gguf(
                    transformer_path.as_path(),
                    &device,
                )
                .map_err(|error| {
                    CandleDiffusionError::load_model(transformer_path.display(), error)
                })?;
                flux::quantized_model::Flux::new(&model_cfg, vb)
                    .map(FluxTransformer::Quantized)
                    .map_err(|error| {
                        CandleDiffusionError::load_model(transformer_path.display(), error)
                    })?
            }
        };

        let t5_config = read_json::<t5::Config>(&t5_config_path)?;
        let t5_vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[t5_path.as_path()], dtype, &device)
                .map_err(|error| CandleDiffusionError::load_model(t5_path.display(), error))?
        };
        let t5 = t5::T5EncoderModel::load(t5_vb, &t5_config)
            .map_err(|error| CandleDiffusionError::load_model(t5_path.display(), error))?;
        let t5_tokenizer = Tokenizer::from_file(&t5_tokenizer_path).map_err(|error| {
            CandleDiffusionError::LoadModel {
                model_path: t5_tokenizer_path.display().to_string(),
                message: format!("failed to load T5 tokenizer: {error}"),
            }
        })?;

        let clip_vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[clip_path.as_path()], dtype, &device)
                .map_err(|error| CandleDiffusionError::load_model(clip_path.display(), error))?
        };
        let clip_config = clip_large_patch14_config();
        let clip =
            clip::text_model::ClipTextTransformer::new(clip_vb.pp("text_model"), &clip_config)
                .map_err(|error| CandleDiffusionError::load_model(clip_path.display(), error))?;
        let clip_tokenizer = Tokenizer::from_file(&clip_tokenizer_path).map_err(|error| {
            CandleDiffusionError::LoadModel {
                model_path: clip_tokenizer_path.display().to_string(),
                message: format!("failed to load CLIP tokenizer: {error}"),
            }
        })?;

        let ae_cfg = match config.flux_model {
            FluxModelKind::Dev => flux::autoencoder::Config::dev(),
            FluxModelKind::Schnell => flux::autoencoder::Config::schnell(),
        };
        let ae_vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[autoencoder_path.as_path()], dtype, &device)
                .map_err(|error| {
                    CandleDiffusionError::load_model(autoencoder_path.display(), error)
                })?
        };
        let autoencoder = flux::autoencoder::AutoEncoder::new(&ae_cfg, ae_vb)
            .map_err(|error| CandleDiffusionError::load_model(autoencoder_path.display(), error))?;

        Ok(Self {
            model_kind: config.flux_model,
            transformer,
            autoencoder,
            t5,
            t5_tokenizer,
            clip,
            clip_tokenizer,
        })
    }

    pub(crate) fn generate(
        &mut self,
        request: &ImageGenerationRequest,
    ) -> Result<GeneratedImage, CandleDiffusionError> {
        if !request.negative_prompt.trim().is_empty() {
            return Err(CandleDiffusionError::UnsupportedOption {
                option: "negative_prompt",
                message: "Flux guidance in Candle 0.10.2 uses a single prompt embedding".to_owned(),
            });
        }
        if request.init_image.is_some()
            || request.mask_image.is_some()
            || request.strength.is_some()
        {
            return Err(CandleDiffusionError::UnsupportedOption {
                option: "img2img",
                message: "Flux pipeline supports text-to-image only in this crate".to_owned(),
            });
        }
        if request.scheduler.is_some() || request.sample_method.is_some() {
            return Err(CandleDiffusionError::UnsupportedOption {
                option: "scheduler",
                message: "Flux uses the schedule from candle-transformers flux::sampling"
                    .to_owned(),
            });
        }

        let device = Device::Cpu;
        device
            .set_seed(request.seed)
            .map_err(|error| CandleDiffusionError::inference(format!("seed RNG: {error}")))?;
        let dtype = DType::F32;
        let height = request.height as usize;
        let width = request.width as usize;
        let prompt = request.prompt.trim();
        let t5_emb = self.t5_embedding(prompt, &device)?;
        let clip_emb = self.clip_embedding(prompt, &device)?;
        let noise = flux::sampling::get_noise(1, height, width, &device)
            .and_then(|tensor| tensor.to_dtype(dtype))
            .map_err(|error| CandleDiffusionError::inference(format!("flux noise: {error}")))?;
        let state = flux::sampling::State::new(&t5_emb, &clip_emb, &noise)
            .map_err(|error| CandleDiffusionError::inference(format!("flux state: {error}")))?;
        let timesteps = match self.model_kind {
            FluxModelKind::Dev => {
                let image_seq_len = state.img.dim(1).map_err(|error| {
                    CandleDiffusionError::inference(format!("flux image sequence length: {error}"))
                })?;
                flux::sampling::get_schedule(request.steps, Some((image_seq_len, 0.5, 1.15)))
            }
            FluxModelKind::Schnell => flux::sampling::get_schedule(request.steps, None),
        };
        let denoised = match &self.transformer {
            FluxTransformer::Normal(model) => flux::sampling::denoise(
                model,
                &state.img,
                &state.img_ids,
                &state.txt,
                &state.txt_ids,
                &state.vec,
                &timesteps,
                request.cfg_scale,
            ),
            FluxTransformer::Quantized(model) => flux::sampling::denoise(
                model,
                &state.img,
                &state.img_ids,
                &state.txt,
                &state.txt_ids,
                &state.vec,
                &timesteps,
                request.cfg_scale,
            ),
        }
        .and_then(|tensor| flux::sampling::unpack(&tensor, height, width))
        .map_err(|error| CandleDiffusionError::inference(format!("flux denoise: {error}")))?;
        let decoded = self
            .autoencoder
            .decode(&denoised)
            .map_err(|error| CandleDiffusionError::inference(format!("flux decode: {error}")))?;
        tensor_to_image(decoded)
    }

    fn t5_embedding(
        &mut self,
        prompt: &str,
        device: &Device,
    ) -> Result<Tensor, CandleDiffusionError> {
        let mut tokens = self
            .t5_tokenizer
            .encode(prompt, true)
            .map_err(|error| CandleDiffusionError::inference(format!("T5 tokenize: {error}")))?
            .get_ids()
            .to_vec();
        tokens.resize(256, 0);
        Tensor::new(tokens.as_slice(), device)
            .and_then(|tensor| tensor.unsqueeze(0))
            .and_then(|tensor| self.t5.forward(&tensor))
            .map_err(|error| CandleDiffusionError::inference(format!("T5 forward: {error}")))
    }

    fn clip_embedding(
        &self,
        prompt: &str,
        device: &Device,
    ) -> Result<Tensor, CandleDiffusionError> {
        let tokens = self
            .clip_tokenizer
            .encode(prompt, true)
            .map_err(|error| CandleDiffusionError::inference(format!("CLIP tokenize: {error}")))?
            .get_ids()
            .to_vec();
        if tokens.len() > 77 {
            return Err(CandleDiffusionError::InvalidParams {
                message: format!("Flux CLIP prompt has {} tokens, max is 77", tokens.len()),
            });
        }
        Tensor::new(tokens.as_slice(), device)
            .and_then(|tensor| tensor.unsqueeze(0))
            .and_then(|tensor| self.clip.forward(&tensor))
            .map_err(|error| CandleDiffusionError::inference(format!("CLIP forward: {error}")))
    }
}

fn flux_transformer_path(config: &CandleDiffusionLoadConfig, root: &Path) -> PathBuf {
    if config.model_path.is_file() {
        return config.model_path.clone();
    }
    match (config.flux_model, config.flux_weight_source) {
        (FluxModelKind::Dev, FluxWeightSource::Safetensors) => root.join("flux1-dev.safetensors"),
        (FluxModelKind::Schnell, FluxWeightSource::Safetensors) => {
            root.join("flux1-schnell.safetensors")
        }
        (FluxModelKind::Dev, FluxWeightSource::QuantizedGguf) => root.join("flux1-dev.gguf"),
        (FluxModelKind::Schnell, FluxWeightSource::QuantizedGguf) => {
            root.join("flux1-schnell.gguf")
        }
    }
}

fn clip_large_patch14_config() -> clip::text_model::ClipTextConfig {
    clip::text_model::ClipTextConfig {
        vocab_size: 49408,
        projection_dim: 768,
        activation: clip::text_model::Activation::QuickGelu,
        intermediate_size: 3072,
        embed_dim: 768,
        max_position_embeddings: 77,
        pad_with: None,
        num_hidden_layers: 12,
        num_attention_heads: 12,
    }
}

fn model_root(model_path: &Path) -> PathBuf {
    if model_path.is_dir() {
        model_path.to_path_buf()
    } else {
        model_path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf()
    }
}

fn required_path(path: PathBuf, label: &'static str) -> Result<PathBuf, CandleDiffusionError> {
    if path.exists() {
        Ok(path)
    } else {
        Err(CandleDiffusionError::InvalidAssetLayout {
            path: path.display().to_string(),
            message: format!("missing {label}"),
        })
    }
}

fn read_json<T>(path: &Path) -> Result<T, CandleDiffusionError>
where
    T: serde::de::DeserializeOwned,
{
    let data = std::fs::read_to_string(path).map_err(|error| CandleDiffusionError::LoadModel {
        model_path: path.display().to_string(),
        message: error.to_string(),
    })?;
    serde_json::from_str(&data).map_err(|error| CandleDiffusionError::LoadModel {
        model_path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn tensor_to_image(image: Tensor) -> Result<GeneratedImage, CandleDiffusionError> {
    let image = image
        .clamp(-1f32, 1f32)
        .and_then(|tensor| tensor + 1.0)
        .and_then(|tensor| tensor * 127.5)
        .and_then(|tensor| tensor.to_dtype(DType::U8))
        .map_err(|error| CandleDiffusionError::inference(format!("flux image tensor: {error}")))?;
    let image = if image.dims().len() == 4 { image.squeeze(0) } else { Ok(image) }
        .map_err(|error| CandleDiffusionError::inference(format!("flux image squeeze: {error}")))?;
    let (channels, height, width) = image
        .dims3()
        .map_err(|error| CandleDiffusionError::inference(format!("flux image dims: {error}")))?;
    let data = image
        .permute((1, 2, 0))
        .and_then(|tensor| tensor.flatten_all())
        .and_then(|tensor| tensor.to_vec1::<u8>())
        .map_err(|error| CandleDiffusionError::inference(format!("flux image data: {error}")))?;
    Ok(GeneratedImage {
        width: width as u32,
        height: height as u32,
        channels: channels as u32,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flux_directory_layout_selects_schnell_weights() {
        let config = CandleDiffusionLoadConfig {
            model_path: PathBuf::from("flux"),
            vae_path: None,
            text_encoder_path: None,
            text_encoder2_path: None,
            tokenizer_path: None,
            tokenizer2_path: None,
            pipeline: crate::config::DiffusionPipelineKind::Flux,
            sd_version: crate::config::StableDiffusionVersion::default(),
            flux_model: FluxModelKind::Schnell,
            flux_weight_source: FluxWeightSource::Safetensors,
            flux_t5_encoder_path: None,
            flux_t5_config_path: None,
            flux_t5_tokenizer_path: None,
            flux_clip_encoder_path: None,
            flux_clip_tokenizer_path: None,
            flux_autoencoder_path: None,
        };
        assert_eq!(
            flux_transformer_path(&config, Path::new("flux")),
            PathBuf::from("flux").join("flux1-schnell.safetensors")
        );
    }
}
