//! Candle-based Stable Diffusion engine adapter.
//!
//! Wraps [`candle_transformers::models::stable_diffusion`] to provide image
//! generation from a text prompt.  When the `candle` feature is disabled all
//! public methods return [`CandleDiffusionEngineError::ModelNotLoaded`].
//!
//! ## Model loading
//!
//! `load_model` validates that the provided weight files exist, then builds and
//! caches the UNet, VAE, and CLIP text-encoder in `InnerState`.  The scheduler
//! is rebuilt per inference call because the number of denoising steps can vary
//! per request.  All subsequent `inference` calls re-use the cached models
//! without any disk I/O, so latency after the first load is minimal.

use std::path::Path;
use std::sync::{Arc, RwLock};

use thiserror::Error;

use crate::engine::EngineError;

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum CandleDiffusionEngineError {
    #[error("model not loaded; call model.load first")]
    ModelNotLoaded,

    #[error("lock poisoned while trying to {operation}")]
    LockPoisoned { operation: &'static str },

    #[error("failed to load model from {model_path}: {message}")]
    LoadModel { model_path: String, message: String },

    #[error("image generation failed: {message}")]
    Inference { message: String },

    #[error("failed to encode output image: {message}")]
    EncodeImage { message: String },

    #[error("invalid generation parameters: {message}")]
    InvalidParams { message: String },
}

// ── Image generation parameters ───────────────────────────────────────────────

/// Parameters for a single image generation request.
#[derive(Debug, Clone)]
pub struct GenImageParams {
    pub prompt: String,
    pub negative_prompt: String,
    pub width: u32,
    pub height: u32,
    pub steps: usize,
    pub cfg_scale: f64,
    pub seed: u64,
}

impl Default for GenImageParams {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            negative_prompt: String::new(),
            width: 512,
            height: 512,
            steps: 20,
            cfg_scale: 7.5,
            seed: 42,
        }
    }
}

// ── Inner state ────────────────────────────────────────────────────────────────

struct InnerState {
    /// Path to the UNet model weights file (kept for diagnostics / reload).
    #[cfg(feature = "candle")]
    model_path: Option<std::path::PathBuf>,
    /// Path to the VAE weight file (may equal `model_path`).
    #[cfg(feature = "candle")]
    vae_path: Option<std::path::PathBuf>,
    /// SD config baked at load time from the requested version.
    /// Kept so the per-inference scheduler can be built from it.
    #[cfg(feature = "candle")]
    sd_config: Option<candle_transformers::models::stable_diffusion::StableDiffusionConfig>,
    /// The tokenizer directory used for CLIP.
    #[cfg(feature = "candle")]
    tokenizer_dir: Option<std::path::PathBuf>,
    /// Cached UNet model built during `load_model`.
    #[cfg(feature = "candle")]
    unet: Option<candle_transformers::models::stable_diffusion::unet_2d::UNet2DConditionModel>,
    /// Cached VAE model built during `load_model`.
    #[cfg(feature = "candle")]
    vae: Option<candle_transformers::models::stable_diffusion::vae::AutoEncoderKL>,
    /// Cached CLIP text-encoder built during `load_model`.
    #[cfg(feature = "candle")]
    clip: Option<candle_transformers::models::stable_diffusion::clip::ClipTextTransformer>,
    /// Cached CLIP tokenizer built during `load_model`.
    #[cfg(feature = "candle")]
    tokenizer: Option<tokenizers::Tokenizer>,

    #[cfg(not(feature = "candle"))]
    _loaded: bool,
}

impl Default for InnerState {
    fn default() -> Self {
        Self {
            #[cfg(feature = "candle")]
            model_path: None,
            #[cfg(feature = "candle")]
            vae_path: None,
            #[cfg(feature = "candle")]
            sd_config: None,
            #[cfg(feature = "candle")]
            tokenizer_dir: None,
            #[cfg(feature = "candle")]
            unet: None,
            #[cfg(feature = "candle")]
            vae: None,
            #[cfg(feature = "candle")]
            clip: None,
            #[cfg(feature = "candle")]
            tokenizer: None,
            #[cfg(not(feature = "candle"))]
            _loaded: false,
        }
    }
}

// ── Engine ────────────────────────────────────────────────────────────────────

/// Engine adapter for Candle-based Stable Diffusion image generation.
#[derive(Clone)]
pub struct CandleDiffusionEngine {
    inner: Arc<RwLock<InnerState>>,
}

impl CandleDiffusionEngine {
    /// Create a new, empty engine (no model loaded).
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(InnerState::default())),
        }
    }

    /// Returns `true` when models are currently loaded and ready for inference.
    pub fn is_model_loaded(&self) -> bool {
        #[cfg(feature = "candle")]
        {
            self.inner
                .read()
                .map(|s| s.unet.is_some())
                .unwrap_or(false)
        }
        #[cfg(not(feature = "candle"))]
        {
            false
        }
    }

    /// Load a Stable Diffusion model.
    ///
    /// `model_path` should point to a safetensors UNet weight file.
    /// `vae_path`, when `None`, reuses the same file for the VAE weights.
    /// `sd_version` selects the architecture: `"v1-5"` or `"v2-1"` (default).
    pub fn load_model(
        &self,
        model_path: &str,
        vae_path: Option<&str>,
        sd_version: &str,
    ) -> Result<(), EngineError> {
        #[cfg(feature = "candle")]
        {
            use candle_core::{DType, Device};
            use candle_transformers::models::stable_diffusion;

            tracing::info!(model_path, sd_version, "loading candle diffusion model");

            let model_file = Path::new(model_path);
            let vae_file_path: std::path::PathBuf;
            let vae_file = if let Some(vp) = vae_path {
                vae_file_path = Path::new(vp).to_path_buf();
                vae_file_path.as_path()
            } else {
                model_file
            };

            // ── Validate that files exist before touching model state ──────────
            if !model_file.exists() {
                return Err(CandleDiffusionEngineError::LoadModel {
                    model_path: model_path.to_owned(),
                    message: "UNet weight file not found".into(),
                }
                .into());
            }
            if !vae_file.exists() {
                return Err(CandleDiffusionEngineError::LoadModel {
                    model_path: vae_file.display().to_string(),
                    message: "VAE weight file not found".into(),
                }
                .into());
            }

            let device = Device::Cpu;
            let dtype = DType::F32;

            // Select SD config based on version string.
            let sd_config = match sd_version {
                "v1-5" => stable_diffusion::StableDiffusionConfig::v1_5(None, None, None),
                _ => stable_diffusion::StableDiffusionConfig::v2_1(None, None, None),
            };

            // Derive tokenizer directory from the model path.
            let tokenizer_dir = model_file
                .parent()
                .unwrap_or(Path::new("."))
                .join("tokenizer");

            // Load and cache CLIP tokenizer + text-encoder.
            let tokenizer_json = tokenizer_dir.join("tokenizer.json");
            let tokenizer =
                tokenizers::Tokenizer::from_file(&tokenizer_json).map_err(|e| {
                    CandleDiffusionEngineError::LoadModel {
                        model_path: tokenizer_json.display().to_string(),
                        message: format!("failed to load CLIP tokenizer: {e}"),
                    }
                })?;

            let clip_weights = tokenizer_dir
                .parent()
                .unwrap_or(&tokenizer_dir)
                .join("text_encoder/model.safetensors");
            let clip = stable_diffusion::build_clip_transformer(
                &sd_config.clip,
                &clip_weights,
                &device,
                dtype,
            )
            .map_err(|e| CandleDiffusionEngineError::LoadModel {
                model_path: clip_weights.display().to_string(),
                message: format!("failed to build CLIP transformer: {e}"),
            })?;

            // Build and cache UNet and VAE.
            let unet = sd_config
                .build_unet(model_file, &device, 4, false, dtype)
                .map_err(|e| CandleDiffusionEngineError::LoadModel {
                    model_path: model_path.to_owned(),
                    message: format!("failed to build UNet: {e}"),
                })?;

            let vae = sd_config
                .build_vae(vae_file, &device, dtype)
                .map_err(|e| CandleDiffusionEngineError::LoadModel {
                    model_path: vae_file.display().to_string(),
                    message: format!("failed to build VAE: {e}"),
                })?;

            let mut state = self.inner.write().map_err(|_| {
                CandleDiffusionEngineError::LockPoisoned {
                    operation: "write diffusion model state",
                }
            })?;
            state.model_path = Some(model_file.to_path_buf());
            state.vae_path = vae_path.map(|p| Path::new(p).to_path_buf());
            state.sd_config = Some(sd_config);
            state.tokenizer_dir = Some(tokenizer_dir);
            state.unet = Some(unet);
            state.vae = Some(vae);
            state.clip = Some(clip);
            state.tokenizer = Some(tokenizer);

            return Ok(());
        }

        #[cfg(not(feature = "candle"))]
        {
            let _ = (model_path, vae_path, sd_version);
            return Err(CandleDiffusionEngineError::ModelNotLoaded.into());
        }
    }

    /// Unload the model and free resources.
    pub fn unload(&self) {
        if let Ok(mut state) = self.inner.write() {
            #[cfg(feature = "candle")]
            {
                state.model_path = None;
                state.vae_path = None;
                state.sd_config = None;
                state.tokenizer_dir = None;
                state.unet = None;
                state.vae = None;
                state.clip = None;
                state.tokenizer = None;
            }
            #[cfg(not(feature = "candle"))]
            {
                state._loaded = false;
            }
        }
    }

    /// Generate an image and return it as PNG bytes.
    ///
    /// Width and height must be multiples of 8.
    pub fn inference(&self, params: &GenImageParams) -> Result<Vec<u8>, EngineError> {
        // Validate dimensions before locking.
        if params.width % 8 != 0 || params.height % 8 != 0 {
            return Err(CandleDiffusionEngineError::InvalidParams {
                message: format!(
                    "width ({}) and height ({}) must be multiples of 8",
                    params.width, params.height
                ),
            }
            .into());
        }
        if params.width == 0 || params.height == 0 {
            return Err(CandleDiffusionEngineError::InvalidParams {
                message: "width and height must be greater than 0".into(),
            }
            .into());
        }

        #[cfg(feature = "candle")]
        {
            self.run_inference(params)
        }

        #[cfg(not(feature = "candle"))]
        {
            let _ = params;
            Err(CandleDiffusionEngineError::ModelNotLoaded.into())
        }
    }

    #[cfg(feature = "candle")]
    fn run_inference(&self, params: &GenImageParams) -> Result<Vec<u8>, EngineError> {
        use candle_core::{DType, Device, IndexOp, Tensor};

        let device = Device::Cpu;
        let _ = DType::F32; // dtype only needed when building models (done at load_model time)

        // Acquire read lock to borrow cached models and config.
        let state = self.inner.read().map_err(|_| CandleDiffusionEngineError::LockPoisoned {
            operation: "read diffusion state for inference",
        })?;

        let (sd_config, unet, vae, clip, tokenizer) = match (
            &state.sd_config,
            &state.unet,
            &state.vae,
            &state.clip,
            &state.tokenizer,
        ) {
            (Some(c), Some(u), Some(v), Some(cl), Some(t)) => (c, u, v, cl, t),
            _ => return Err(CandleDiffusionEngineError::ModelNotLoaded.into()),
        };

        // Encode prompt and negative prompt via CLIP tokenizer.
        let encode_text = |text: &str| -> Result<Tensor, CandleDiffusionEngineError> {
            let enc = tokenizer.encode(text, true).map_err(|e| {
                CandleDiffusionEngineError::Inference {
                    message: format!("tokenize failed: {e}"),
                }
            })?;
            let ids: Vec<i64> = enc.get_ids().iter().map(|&id| id as i64).collect();
            Tensor::new(ids.as_slice(), &device)
                .and_then(|t| t.unsqueeze(0))
                .map_err(|e| CandleDiffusionEngineError::Inference {
                    message: format!("token tensor: {e}"),
                })
        };

        let prompt_ids = encode_text(&params.prompt)?;
        let uncond_ids = encode_text(&params.negative_prompt)?;

        // Generate text embeddings via cached CLIP encoder.
        let prompt_embeds =
            clip.forward_with_mask(&prompt_ids, prompt_ids.dim(1).unwrap_or(0))
                .map_err(|e| CandleDiffusionEngineError::Inference {
                    message: format!("CLIP forward (prompt): {e}"),
                })?;
        let uncond_embeds =
            clip.forward_with_mask(&uncond_ids, uncond_ids.dim(1).unwrap_or(0))
                .map_err(|e| CandleDiffusionEngineError::Inference {
                    message: format!("CLIP forward (uncond): {e}"),
                })?;
        let text_embeddings =
            Tensor::cat(&[uncond_embeds, prompt_embeds], 0).map_err(|e| {
                CandleDiffusionEngineError::Inference {
                    message: format!("embed cat: {e}"),
                }
            })?;

        // Build the scheduler (depends on params.steps which varies per call).
        let mut scheduler =
            sd_config
                .build_scheduler(params.steps)
                .map_err(|e| CandleDiffusionEngineError::Inference {
                    message: format!("Scheduler build failed: {e}"),
                })?;

        // Noise initialisation with seed-derived deterministic RNG.
        let bsize = 1usize;
        let latent_h = (params.height / 8) as usize;
        let latent_w = (params.width / 8) as usize;

        device.set_seed(params.seed).map_err(|e| CandleDiffusionEngineError::Inference {
            message: format!("failed to seed RNG: {e}"),
        })?;
        let mut latents = Tensor::randn(
            0.0f32,
            1.0f32,
            (bsize, 4, latent_h, latent_w),
            &device,
        )
        .map_err(|e| CandleDiffusionEngineError::Inference {
            message: format!("noise tensor: {e}"),
        })?;

        let init_noise_sigma = scheduler.init_noise_sigma();
        latents = (latents * init_noise_sigma).map_err(|e| CandleDiffusionEngineError::Inference {
            message: format!("scale noise: {e}"),
        })?;

        // Collect timesteps first to avoid simultaneous mut/immut borrows on scheduler.
        let timesteps: Vec<usize> = scheduler.timesteps().to_vec();
        let total_steps = timesteps.len();

        // Denoising loop.
        for (idx, t) in timesteps.iter().enumerate() {
            let latent_model_input = Tensor::cat(&[&latents, &latents], 0).map_err(|e| {
                CandleDiffusionEngineError::Inference {
                    message: format!("latent cat: {e}"),
                }
            })?;
            let latent_model_input =
                scheduler
                    .scale_model_input(latent_model_input, *t)
                    .map_err(|e| CandleDiffusionEngineError::Inference {
                        message: format!("scale model input: {e}"),
                    })?;

            let noise_pred = unet
                .forward(&latent_model_input, *t as f64, &text_embeddings)
                .map_err(|e| CandleDiffusionEngineError::Inference {
                    message: format!("unet forward: {e}"),
                })?;

            let noise_pred_uncond =
                noise_pred.i(..bsize).map_err(|e| CandleDiffusionEngineError::Inference {
                    message: format!("slice uncond: {e}"),
                })?;
            let noise_pred_text =
                noise_pred.i(bsize..).map_err(|e| CandleDiffusionEngineError::Inference {
                    message: format!("slice text: {e}"),
                })?;

            let guided = (&noise_pred_text - &noise_pred_uncond)
                .map_err(|e| CandleDiffusionEngineError::Inference {
                    message: format!("guidance sub: {e}"),
                })?
                * params.cfg_scale;
            let guided = guided.map_err(|e| CandleDiffusionEngineError::Inference {
                message: format!("guidance scale: {e}"),
            })?;
            let noise_pred =
                (&noise_pred_uncond + guided).map_err(|e| CandleDiffusionEngineError::Inference {
                    message: format!("guidance add: {e}"),
                })?;

            latents =
                scheduler
                    .step(&noise_pred, *t, &latents)
                    .map_err(|e| CandleDiffusionEngineError::Inference {
                        message: format!("scheduler step: {e}"),
                    })?;

            tracing::debug!(
                step = idx,
                total = scheduler.timesteps().len(),
                "candle diffusion step"
            );
        }

        // Decode latents → image.
        let decoded = vae
            .decode(
                &(latents / 0.18215f64).map_err(|e| CandleDiffusionEngineError::Inference {
                    message: format!("latent scale: {e}"),
                })?,
            )
            .map_err(|e| CandleDiffusionEngineError::Inference {
                message: format!("vae decode: {e}"),
            })?;
        let decoded = ((decoded / 2.0)
            .map_err(|e| CandleDiffusionEngineError::Inference {
                message: format!("img div: {e}"),
            })?
            + 0.5f64)
            .map_err(|e| CandleDiffusionEngineError::Inference {
                message: format!("img add: {e}"),
            })?
            .clamp(0.0, 1.0)
            .map_err(|e| CandleDiffusionEngineError::Inference {
                message: format!("clamp: {e}"),
            })?;

        let image_u8 = (decoded * 255.0)
            .map_err(|e| CandleDiffusionEngineError::Inference {
                message: format!("scale u8: {e}"),
            })?
            .to_dtype(DType::U8)
            .map_err(|e| CandleDiffusionEngineError::Inference {
                message: format!("to u8: {e}"),
            })?;

        // Convert to PNG bytes.
        let (_channels, h, w) = image_u8.dims3().map_err(|e| CandleDiffusionEngineError::Inference {
            message: format!("dims: {e}"),
        })?;
        let data = image_u8
            .permute((1, 2, 0))
            .map_err(|e| CandleDiffusionEngineError::Inference {
                message: format!("permute: {e}"),
            })?
            .flatten_all()
            .map_err(|e| CandleDiffusionEngineError::Inference {
                message: format!("flatten: {e}"),
            })?
            .to_vec1::<u8>()
            .map_err(|e| CandleDiffusionEngineError::Inference {
                message: format!("to_vec: {e}"),
            })?;

        let mut png_bytes: Vec<u8> = Vec::new();
        let img_buffer = image::RgbImage::from_raw(w as u32, h as u32, data)
            .ok_or_else(|| CandleDiffusionEngineError::EncodeImage {
                message: "image buffer construction failed".into(),
            })?;
        img_buffer
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .map_err(|e| CandleDiffusionEngineError::EncodeImage {
                message: format!("png encode: {e}"),
            })?;

        Ok(png_bytes)
    }
}

impl Default for CandleDiffusionEngine {
    fn default() -> Self {
        Self::new()
    }
}
