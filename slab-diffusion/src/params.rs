use std::ffi::CString;
use std::ptr;

/// Weight / quantization type for the model weights.
pub use slab_diffusion_sys::sd_type_t as WeightType;

/// Random number generator type.
pub use slab_diffusion_sys::rng_type_t as RngType;

/// Denoising sampling method.
pub use slab_diffusion_sys::sample_method_t as SampleMethod;

/// Sigma schedule (scheduler).
pub use slab_diffusion_sys::scheduler_t as Scheduler;

/// Model prediction type override.
pub use slab_diffusion_sys::prediction_t as Prediction;

/// LoRA apply mode.
pub use slab_diffusion_sys::lora_apply_mode_t as LoraApplyMode;

/// Cache mode for inference acceleration.
pub use slab_diffusion_sys::sd_cache_mode_t as CacheMode;

/// Log level emitted by the native library.
pub use slab_diffusion_sys::sd_log_level_t as SdLogLevel;

pub use slab_diffusion_sys::{
    lora_apply_mode_t_LORA_APPLY_AUTO as LORA_APPLY_AUTO,
    prediction_t_PREDICTION_COUNT as PREDICTION_COUNT,
    rng_type_t_CPU_RNG as RNG_CPU,
    rng_type_t_CUDA_RNG as RNG_CUDA,
    rng_type_t_RNG_TYPE_COUNT as RNG_TYPE_COUNT,
    rng_type_t_STD_DEFAULT_RNG as RNG_STD_DEFAULT,
    sample_method_t_DPM2_SAMPLE_METHOD as SAMPLE_DPM2,
    sample_method_t_DPMPP2M_SAMPLE_METHOD as SAMPLE_DPM_PP_2M,
    sample_method_t_DPMPP2Mv2_SAMPLE_METHOD as SAMPLE_DPM_PP_2M_V2,
    sample_method_t_DPMPP2S_A_SAMPLE_METHOD as SAMPLE_DPM_PP_2S_A,
    sample_method_t_EULER_A_SAMPLE_METHOD as SAMPLE_EULER_A,
    sample_method_t_EULER_SAMPLE_METHOD as SAMPLE_EULER,
    sample_method_t_HEUN_SAMPLE_METHOD as SAMPLE_HEUN,
    sample_method_t_IPNDM_SAMPLE_METHOD as SAMPLE_IPNDM,
    sample_method_t_IPNDM_V_SAMPLE_METHOD as SAMPLE_IPNDM_V,
    sample_method_t_LCM_SAMPLE_METHOD as SAMPLE_LCM,
    sample_method_t_SAMPLE_METHOD_COUNT as SAMPLE_METHOD_COUNT,
    scheduler_t_AYS_SCHEDULER as SCHEDULER_AYS,
    scheduler_t_DISCRETE_SCHEDULER as SCHEDULER_DISCRETE,
    scheduler_t_EXPONENTIAL_SCHEDULER as SCHEDULER_EXPONENTIAL,
    scheduler_t_GITS_SCHEDULER as SCHEDULER_GITS,
    scheduler_t_KARRAS_SCHEDULER as SCHEDULER_KARRAS,
    scheduler_t_SCHEDULER_COUNT as SCHEDULER_COUNT,
    sd_cache_mode_t_SD_CACHE_DISABLED as CACHE_DISABLED,
    sd_type_t_SD_TYPE_COUNT as WEIGHT_TYPE_AUTO,
};

/// Convert a non-empty Rust `&str` to a `CString`.
///
/// Returns `None` for empty strings, which the C API treats as "not provided".
pub(crate) fn opt_cstring(s: &str) -> Option<CString> {
    if s.is_empty() {
        None
    } else {
        Some(CString::new(s).expect("opt_cstring: string contains an interior null byte"))
    }
}

/// Return the pointer of a `CString`, or null when the option is `None`.
pub(crate) fn ptr_or_null(cs: &Option<CString>) -> *const std::os::raw::c_char {
    cs.as_ref().map_or(ptr::null(), |s| s.as_ptr())
}

/// Parameters used when constructing a [`crate::SdContext`].
///
/// All path fields default to an empty string (treated as "not provided").
/// This mirrors the native `sd_ctx_params_t` fields exposed by
/// stable-diffusion.cpp.
#[derive(Debug, Clone)]
pub struct SdContextParams {
    /// Path to a full model file (e.g. `.gguf` / `.safetensors`).
    pub model_path: String,

    /// Path to the standalone diffusion model (alternative to `model_path`).
    pub diffusion_model_path: String,

    /// Path to the CLIP-L text encoder.
    pub clip_l_path: String,

    /// Path to the CLIP-G text encoder.
    pub clip_g_path: String,

    /// Path to the T5-XXL text encoder.
    pub t5xxl_path: String,

    /// Path to the LLM (Qwen2VL / etc.) text encoder.
    pub llm_path: String,

    /// Path to the LLM vision encoder.
    pub llm_vision_path: String,

    /// Path to the CLIP vision encoder.
    pub clip_vision_path: String,

    /// Path to the high-noise diffusion model.
    pub high_noise_diffusion_model_path: String,

    /// Path to the VAE model.
    pub vae_path: String,

    /// Path to a TAESD decoder for preview decoding.
    pub taesd_path: String,

    /// Path to a ControlNet model.
    pub control_net_path: String,

    /// Path to a PhotoMaker model.
    pub photo_maker_path: String,

    /// Number of CPU threads to use. `0` means auto.
    pub n_threads: i32,

    /// Weight / quantization type. [`WEIGHT_TYPE_AUTO`] means "same as file".
    pub weight_type: WeightType,

    /// RNG used for noise generation.
    pub rng_type: RngType,

    /// Prediction type override. [`PREDICTION_COUNT`] means auto.
    pub prediction: Prediction,

    /// LoRA application mode.
    pub lora_apply_mode: LoraApplyMode,

    /// Keep the VAE model in CPU RAM to save VRAM.
    pub keep_vae_on_cpu: bool,

    /// Keep the CLIP model(s) in CPU RAM to save VRAM.
    pub keep_clip_on_cpu: bool,

    /// Keep the ControlNet model in CPU RAM to save VRAM.
    pub keep_control_net_on_cpu: bool,

    /// Offload model parameters to CPU and load on demand into GPU.
    pub offload_params_to_cpu: bool,

    /// Enable memory-mapped model loading.
    pub enable_mmap: bool,

    /// Skip building the VAE encode graph when only doing txt2img.
    pub vae_decode_only: bool,

    /// Use the TAESD decoder for preview only.
    pub taesd_preview_only: bool,

    /// Enable Flash Attention in the text encoder.
    pub flash_attn: bool,

    /// Enable Flash Attention in the diffusion model.
    pub diffusion_flash_attn: bool,
}

impl Default for SdContextParams {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            diffusion_model_path: String::new(),
            clip_l_path: String::new(),
            clip_g_path: String::new(),
            t5xxl_path: String::new(),
            llm_path: String::new(),
            llm_vision_path: String::new(),
            clip_vision_path: String::new(),
            high_noise_diffusion_model_path: String::new(),
            vae_path: String::new(),
            taesd_path: String::new(),
            control_net_path: String::new(),
            photo_maker_path: String::new(),
            n_threads: 0,
            weight_type: WEIGHT_TYPE_AUTO,
            rng_type: RNG_STD_DEFAULT,
            prediction: PREDICTION_COUNT,
            lora_apply_mode: LORA_APPLY_AUTO,
            keep_vae_on_cpu: false,
            keep_clip_on_cpu: false,
            keep_control_net_on_cpu: false,
            offload_params_to_cpu: false,
            enable_mmap: false,
            vae_decode_only: true,
            taesd_preview_only: false,
            flash_attn: false,
            diffusion_flash_attn: false,
        }
    }
}

impl SdContextParams {
    /// Convenience constructor that only sets the full-model path.
    pub fn with_model(model_path: impl Into<String>) -> Self {
        Self { model_path: model_path.into(), ..Default::default() }
    }
}

/// Parameters passed to [`crate::SdContext::generate_image`].
///
/// Maps to the native `sd_img_gen_params_t` struct, including nested
/// `sd_sample_params_t` fields like `flow_shift`.
#[derive(Debug, Clone)]
pub struct SdImgGenParams {
    /// Positive prompt.
    pub prompt: String,

    /// Negative prompt.
    pub negative_prompt: String,

    /// CLIP skip: number of CLIP tail layers to ignore. `0` means auto.
    pub clip_skip: i32,

    /// Output image width in pixels.
    pub width: u32,

    /// Output image height in pixels.
    pub height: u32,

    /// Classifier-Free Guidance scale.
    pub cfg_scale: f32,

    /// Distilled guidance scale used by models with a guidance input.
    pub guidance: f32,

    /// Number of denoising steps.
    pub sample_steps: i32,

    /// Sampling method. [`SAMPLE_METHOD_COUNT`] means auto.
    pub sample_method: SampleMethod,

    /// Sigma schedule. [`SCHEDULER_COUNT`] means auto.
    pub scheduler: Scheduler,

    /// DDIM/TCD/RES eta value.
    pub eta: f32,

    /// Flow shift for samplers that support it. `f32::INFINITY` means auto.
    pub flow_shift: f32,

    /// RNG seed. Negative values choose a random seed.
    pub seed: i64,

    /// Number of images to produce in a single call.
    pub batch_count: i32,

    /// Strength of the init-image influence for img2img / inpainting.
    pub strength: f32,

    /// Optional initial image for img2img / video generation.
    pub init_image: Option<SdImage>,
}

impl Default for SdImgGenParams {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            negative_prompt: String::new(),
            clip_skip: 0,
            width: 512,
            height: 512,
            cfg_scale: 7.0,
            guidance: 3.5,
            sample_steps: 20,
            sample_method: SAMPLE_METHOD_COUNT,
            scheduler: SCHEDULER_COUNT,
            eta: 0.0,
            flow_shift: f32::INFINITY,
            seed: 42,
            batch_count: 1,
            strength: 0.75,
            init_image: None,
        }
    }
}

impl SdImgGenParams {
    /// Convenience constructor that only sets the prompt.
    pub fn with_prompt(prompt: impl Into<String>) -> Self {
        Self { prompt: prompt.into(), ..Default::default() }
    }
}

/// A generated image returned by [`crate::SdContext::generate_image`].
///
/// Pixel data is stored in row-major, channel-last (HWC) order.
#[derive(Debug, Clone)]
pub struct SdImage {
    /// Image width in pixels.
    pub width: u32,

    /// Image height in pixels.
    pub height: u32,

    /// Number of channels per pixel, usually 3 for RGB.
    pub channel: u32,

    /// Raw pixel data (`width * height * channel` bytes).
    pub data: Vec<u8>,
}
