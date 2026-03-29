//! Shared deserialization types for GGML backend worker configuration.
//!
//! These structs are used by all three GGML backends (llama, whisper, diffusion)
//! for their `model.load` JSON input payloads.

use serde::Deserialize;

/// Input configuration for `model.load` operations (all backends).
#[derive(Deserialize)]
pub(crate) struct ModelLoadConfig {
    pub model_path: String,
}

/// Input configuration for `model.load` operations specific to the diffusion backend.
///
/// Extends [`ModelLoadConfig`] with optional context parameters that control
/// which auxiliary model files are loaded alongside the main model.
#[derive(Deserialize, Default)]
pub(crate) struct DiffusionModelLoadConfig {
    pub model_path: String,

    // ── Optional auxiliary model paths ───────────────────────────────────────
    #[serde(default)]
    pub diffusion_model_path: String,
    #[serde(default)]
    pub vae_path: String,
    #[serde(default)]
    pub taesd_path: String,
    #[serde(default)]
    pub clip_l_path: String,
    #[serde(default)]
    pub clip_g_path: String,
    #[serde(default)]
    pub t5xxl_path: String,
    #[serde(default)]
    pub clip_vision_path: String,
    #[serde(default)]
    pub control_net_path: String,

    // ── Performance / memory flags ───────────────────────────────────────────
    #[serde(default)]
    pub flash_attn: bool,
    #[serde(default)]
    pub vae_device: String,
    #[serde(default)]
    pub clip_device: String,
    #[serde(default)]
    pub offload_params_to_cpu: bool,
    #[serde(default)]
    pub enable_mmap: bool,
    #[serde(default = "default_threads")]
    pub n_threads: i32,
}

fn default_threads() -> i32 {
    0 // 0 = auto (physical core count)
}
