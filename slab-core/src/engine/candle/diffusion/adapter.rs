//! Candle-based Stable Diffusion engine adapter.
//!
//! Wraps [`candle_transformers::models::stable_diffusion`] to provide image
//! generation from a text prompt.  When the `candle` feature is disabled all
//! public methods return [`CandleDiffusionEngineError::ModelNotLoaded`].

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

#[derive(Default)]
struct InnerState {
    /// Stable diffusion pipeline; `None` when no model is loaded.
    #[cfg(feature = "candle")]
    pipeline: Option<Box<dyn CandleDiffusionPipeline + Send + Sync>>,
    #[cfg(not(feature = "candle"))]
    pipeline: Option<()>,
}

// ── Pipeline trait (candle feature only) ─────────────────────────────────────

#[cfg(feature = "candle")]
trait CandleDiffusionPipeline {
    fn generate(&mut self, params: &GenImageParams) -> Result<Vec<u8>, String>;
}

#[cfg(feature = "candle")]
struct StableDiffusionPipelineWrapper {
    config: candle_transformers::models::stable_diffusion::StableDiffusionConfig,
    model_path: String,
    vae_path: Option<String>,
}

#[cfg(feature = "candle")]
impl CandleDiffusionPipeline for StableDiffusionPipelineWrapper {
    fn generate(&mut self, params: &GenImageParams) -> Result<Vec<u8>, String> {
        use candle_core::{DType, Device, Tensor};
        use candle_transformers::models::stable_diffusion;

        let device = Device::Cpu;
        let dtype = DType::F32;

        // Build the full pipeline on-the-fly for each generation.  This is
        // memory-inefficient but avoids storing Send-unsafe Candle state.
        let sd_config = &self.config;

        let tokenizer_path = Path::new(&self.model_path)
            .parent()
            .unwrap_or(Path::new("."))
            .join("tokenizer")
            .join("tokenizer.json");

        let tokenizer = candle_transformers::models::stable_diffusion::build_clip_transformer(
            &sd_config.clip,
            tokenizer_path.to_str().unwrap_or(""),
            &device,
            dtype,
        )
        .map_err(|e| format!("CLIP tokenizer/encoder load failed: {e}"))?;

        let unet_weights = candle_core::safetensors::load(&self.model_path, &device)
            .map_err(|e| format!("UNet weights load failed: {e}"))?;
        let unet_vb =
            candle_nn::VarBuilder::from_tensors(unet_weights, dtype, &device);
        let unet = sd_config
            .build_unet(unet_vb, 4)
            .map_err(|e| format!("UNet build failed: {e}"))?;

        let vae_path = self
            .vae_path
            .as_deref()
            .unwrap_or(self.model_path.as_str());
        let vae_weights = candle_core::safetensors::load(vae_path, &device)
            .map_err(|e| format!("VAE weights load failed: {e}"))?;
        let vae_vb = candle_nn::VarBuilder::from_tensors(vae_weights, dtype, &device);
        let vae = sd_config
            .build_vae(vae_vb, &device, dtype)
            .map_err(|e| format!("VAE build failed: {e}"))?;

        let scheduler =
            sd_config.build_scheduler(params.steps).map_err(|e| format!("Scheduler build failed: {e}"))?;

        // Encode text prompt.
        let prompt_ids = candle_transformers::models::stable_diffusion::get_text_embeddings(
            &params.prompt,
            &tokenizer,
            0,
            100,
            false,
        )
        .map_err(|e| format!("text embed failed: {e}"))?;
        let uncond_ids = candle_transformers::models::stable_diffusion::get_text_embeddings(
            &params.negative_prompt,
            &tokenizer,
            0,
            100,
            false,
        )
        .map_err(|e| format!("uncond embed failed: {e}"))?;
        let text_embeddings = Tensor::cat(&[uncond_ids, prompt_ids], 0)
            .map_err(|e| format!("embed cat: {e}"))?;

        // Noise initialisation.
        let bsize = 1usize;
        let latent_size = (params.height / 8) as usize;
        let latent_width = (params.width / 8) as usize;

        let mut rng = candle_core::Tensor::randn(
            0.0f32,
            1.0f32,
            (bsize, 4, latent_size, latent_width),
            &device,
        )
        .map_err(|e| format!("randn: {e}"))?;

        let init_noise_sigma = scheduler.init_noise_sigma();
        rng = (rng * init_noise_sigma).map_err(|e| format!("scale noise: {e}"))?;

        // Denoising loop.
        for (idx, t) in scheduler.timesteps().iter().enumerate() {
            let latent_model_input = Tensor::cat(&[&rng, &rng], 0)
                .map_err(|e| format!("latent cat: {e}"))?;
            let latent_model_input = scheduler
                .scale_model_input(latent_model_input, *t)
                .map_err(|e| format!("scale: {e}"))?;

            let noise_pred = unet
                .forward(&latent_model_input, *t as f64, &text_embeddings)
                .map_err(|e| format!("unet forward: {e}"))?;

            let noise_pred_uncond = noise_pred.i(..bsize).map_err(|e| format!("slice: {e}"))?;
            let noise_pred_text = noise_pred.i(bsize..).map_err(|e| format!("slice: {e}"))?;

            let noise_pred = (noise_pred_uncond
                + (noise_pred_text - &noise_pred_uncond).map_err(|e| format!("sub: {e}"))?
                    * params.cfg_scale)
                .map_err(|e| format!("guidance: {e}"))?;

            rng = scheduler
                .step(&noise_pred, *t, &rng)
                .map_err(|e| format!("scheduler step: {e}"))?;

            tracing::debug!(
                step = idx,
                total = scheduler.timesteps().len(),
                "candle diffusion step"
            );
        }

        // Decode latents to image.
        let image = vae
            .decode(&(&rng / 0.18215f64).map_err(|e| format!("scale: {e}"))?)
            .map_err(|e| format!("vae decode: {e}"))?;
        let image = ((image / 2.0).map_err(|e| format!("img div: {e}"))?
            + 0.5f64)
            .map_err(|e| format!("img add: {e}"))?
            .clamp(0.0, 1.0)
            .map_err(|e| format!("clamp: {e}"))?;

        let image = (image * 255.0)
            .map_err(|e| format!("scale: {e}"))?
            .to_dtype(DType::U8)
            .map_err(|e| format!("u8: {e}"))?;

        // Convert to PNG bytes.
        let (c, h, w) = image
            .dims3()
            .map_err(|e| format!("dims: {e}"))?;
        let data = image
            .permute((1, 2, 0))
            .map_err(|e| format!("permute: {e}"))?
            .flatten_all()
            .map_err(|e| format!("flatten: {e}"))?
            .to_vec1::<u8>()
            .map_err(|e| format!("to_vec: {e}"))?;

        let mut png_bytes: Vec<u8> = Vec::new();
        let img_buffer: image::RgbImage = image::RgbImage::from_raw(w as u32, h as u32, data)
            .ok_or_else(|| "image buffer construction failed".to_owned())?;
        img_buffer
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .map_err(|e| format!("png encode: {e}"))?;

        Ok(png_bytes)
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

    /// Returns `true` when a model is currently loaded.
    pub fn is_model_loaded(&self) -> bool {
        self.inner
            .read()
            .map(|s| s.pipeline.is_some())
            .unwrap_or(false)
    }

    /// Load a Stable Diffusion model.
    ///
    /// `model_path` should point to a safetensors UNet weight file.
    pub fn load_model(
        &self,
        model_path: &str,
        vae_path: Option<&str>,
    ) -> Result<(), EngineError> {
        #[cfg(feature = "candle")]
        {
            use candle_transformers::models::stable_diffusion;

            tracing::info!(model_path, "loading candle diffusion model");

            let config =
                stable_diffusion::StableDiffusionConfig::v2_1(None, None, None);

            let mut state = self.inner.write().map_err(|_| {
                CandleDiffusionEngineError::LockPoisoned {
                    operation: "write diffusion model state",
                }
            })?;

            state.pipeline = Some(Box::new(StableDiffusionPipelineWrapper {
                config,
                model_path: model_path.to_owned(),
                vae_path: vae_path.map(str::to_owned),
            }));
        }

        #[cfg(not(feature = "candle"))]
        {
            let _ = (model_path, vae_path);
            tracing::warn!(
                "candle feature is not enabled; model.load is a no-op for CandleDiffusionEngine"
            );
        }

        Ok(())
    }

    /// Unload the model and free resources.
    pub fn unload(&self) {
        if let Ok(mut state) = self.inner.write() {
            state.pipeline = None;
        }
    }

    /// Generate an image and return it as PNG bytes.
    pub fn inference(&self, params: &GenImageParams) -> Result<Vec<u8>, EngineError> {
        #[cfg(feature = "candle")]
        {
            let mut state = self.inner.write().map_err(|_| {
                CandleDiffusionEngineError::LockPoisoned {
                    operation: "lock diffusion state for inference",
                }
            })?;

            let pipeline = state
                .pipeline
                .as_mut()
                .ok_or(CandleDiffusionEngineError::ModelNotLoaded)?;

            pipeline
                .generate(params)
                .map_err(|message| CandleDiffusionEngineError::Inference { message }.into())
        }

        #[cfg(not(feature = "candle"))]
        {
            let _ = params;
            Err(CandleDiffusionEngineError::ModelNotLoaded.into())
        }
    }
}

impl Default for CandleDiffusionEngine {
    fn default() -> Self {
        Self::new()
    }
}
