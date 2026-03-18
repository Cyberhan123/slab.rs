//! Shared deserialisation types for Candle backend worker configuration.
//!
//! Unlike the GGML backends that load dynamic libraries at runtime, the Candle
//! backends are statically linked.  These structs cover only the model-loading
//! and inference configuration payloads; there is no `LibLoadConfig` equivalent.

use serde::Deserialize;

/// Input configuration for `model.load` operations (all Candle backends).
#[derive(Deserialize)]
pub(crate) struct CandleModelLoadConfig {
    /// Path to the primary model weight file (GGUF, safetensors, or bin).
    pub model_path: String,
    /// Optional path to a separate tokenizer JSON file.
    ///
    /// When omitted the backend will look for `tokenizer.json` in the same
    /// directory as `model_path`.
    #[serde(default)]
    pub tokenizer_path: Option<String>,
    /// Optional revision / branch to fetch from HuggingFace Hub when a model
    /// ID is given instead of a local file path.  Ignored for local paths.
    #[serde(default)]
    pub revision: Option<String>,
}

/// Extended model-load config for the Candle Llama backend.
#[derive(Deserialize)]
pub(crate) struct CandleLlamaModelLoadConfig {
    /// Path to the primary model weight file (GGUF format).
    pub model_path: String,
    /// Optional tokenizer path; falls back to `<model_dir>/tokenizer.json`.
    #[serde(default)]
    pub tokenizer_path: Option<String>,
    /// Seed for the random number generator.
    ///
    /// `0` is a valid deterministic seed.  To get non-reproducible output
    /// generate a random `u64` before submitting the request.
    #[serde(default)]
    pub seed: u64,
}

/// Extended model-load config for the Candle diffusion backend.
#[derive(Deserialize, Default)]
pub(crate) struct CandleDiffusionModelLoadConfig {
    /// Path to the UNet / single-file diffusion model weights (safetensors).
    pub model_path: String,
    /// Optional path to a VAE weight file.
    #[serde(default)]
    pub vae_path: Option<String>,
    /// Stable Diffusion version.  Accepted values: `"v1-5"`, `"v2-1"` (default).
    #[serde(default = "default_sd_version")]
    pub sd_version: String,
}

fn default_sd_version() -> String {
    "v2-1".to_owned()
}
