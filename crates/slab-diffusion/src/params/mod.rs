mod cache;
mod context;
mod embedding;
mod guidance;
mod image;
mod lora;
mod pm;
mod prediction;
mod sampler;
mod scheduler;
mod slg;
mod support;
mod vae_tiling;
mod video_params;

use serde::{Deserialize, Serialize};

pub(crate) use cache::InnerCacheParams;
pub(crate) use context::InnerContextParams;
pub(crate) use image::owned_image_from_raw;
pub(crate) use image::InnerImgParams;
pub(crate) use pm::InnerPmParams;
pub(crate) use sampler::InnerSampleParams;
pub(crate) use support::image_view;
pub(crate) use video_params::InnerVideoParams;

pub use cache::CacheParams;
pub use context::ContextParams;
pub use embedding::Embedding;
pub use guidance::GuidanceParams;
pub use image::{Image, ImgParams};
pub use lora::{Lora, LoraApplyMode};
pub use pm::PmParams;
pub use prediction::Prediction;
pub use sampler::{SampleMethod, SampleParams};
pub use scheduler::Scheduler;
/// Log level emitted by the native library.
use slab_diffusion_sys::sd_log_level_t;
pub use slg::SlgParams;
pub use vae_tiling::TilingParams;
pub use video_params::{Video, VideoParams};

// log level parameters must keep code order
#[rustfmt::skip]
use slab_diffusion_sys::{
    sd_log_level_t_SD_LOG_DEBUG,
    sd_log_level_t_SD_LOG_INFO,
    sd_log_level_t_SD_LOG_WARN,
    sd_log_level_t_SD_LOG_ERROR,
};

#[cfg_attr(any(not(windows), target_env = "gnu"), repr(u32))] // include windows-gnu
#[cfg_attr(all(windows, not(target_env = "gnu")), repr(i32))] // msvc being *special* again
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LogLevel {
    Debug = sd_log_level_t_SD_LOG_DEBUG,
    Info = sd_log_level_t_SD_LOG_INFO,
    Warn = sd_log_level_t_SD_LOG_WARN,
    Error = sd_log_level_t_SD_LOG_ERROR,
}

impl From<LogLevel> for sd_log_level_t {
    fn from(value: LogLevel) -> Self {
        value as Self
    }
}

/// RNG type for noise generation.
use slab_diffusion_sys::rng_type_t;
// RNG type parameters must keep code order
#[rustfmt::skip]
use slab_diffusion_sys::{
    rng_type_t_STD_DEFAULT_RNG,
    rng_type_t_CUDA_RNG,
    rng_type_t_CPU_RNG,
    rng_type_t_RNG_TYPE_COUNT,
};

#[cfg_attr(any(not(windows), target_env = "gnu"), repr(u32))] // include windows-gnu
#[cfg_attr(all(windows, not(target_env = "gnu")), repr(i32))] // msvc being *special* again
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RngType {
    Default = rng_type_t_STD_DEFAULT_RNG,
    Cuda = rng_type_t_CUDA_RNG,
    Cpu = rng_type_t_CPU_RNG,
    Unknown = rng_type_t_RNG_TYPE_COUNT,
}

impl From<RngType> for rng_type_t {
    fn from(value: RngType) -> Self {
        value as Self
    }
}

impl Default for RngType {
    fn default() -> Self {
        Self::Default
    }
}

/// Weight / quantization type for the model weights.
pub use slab_diffusion_sys::sd_type_t;
// weight type parameters must keep code order
#[rustfmt::skip]
use slab_diffusion_sys::{
    sd_type_t_SD_TYPE_F32,
    sd_type_t_SD_TYPE_F16,
    sd_type_t_SD_TYPE_Q4_0,
    sd_type_t_SD_TYPE_Q4_1,
    sd_type_t_SD_TYPE_Q5_0,
    sd_type_t_SD_TYPE_Q5_1,
    sd_type_t_SD_TYPE_Q8_0,
    sd_type_t_SD_TYPE_Q8_1,
    sd_type_t_SD_TYPE_Q2_K,
    sd_type_t_SD_TYPE_Q3_K,
    sd_type_t_SD_TYPE_Q4_K,
    sd_type_t_SD_TYPE_Q5_K,
    sd_type_t_SD_TYPE_Q6_K,
    sd_type_t_SD_TYPE_Q8_K,
    sd_type_t_SD_TYPE_IQ2_XXS,
    sd_type_t_SD_TYPE_IQ2_XS,
    sd_type_t_SD_TYPE_IQ3_XXS,
    sd_type_t_SD_TYPE_IQ1_S,
    sd_type_t_SD_TYPE_IQ4_NL,
    sd_type_t_SD_TYPE_IQ3_S,
    sd_type_t_SD_TYPE_IQ2_S,
    sd_type_t_SD_TYPE_IQ4_XS,
    sd_type_t_SD_TYPE_I8,
    sd_type_t_SD_TYPE_I16,
    sd_type_t_SD_TYPE_I32,
    sd_type_t_SD_TYPE_I64,
    sd_type_t_SD_TYPE_F64,
    sd_type_t_SD_TYPE_IQ1_M,
    sd_type_t_SD_TYPE_BF16,
    sd_type_t_SD_TYPE_TQ1_0,
    sd_type_t_SD_TYPE_TQ2_0,
    sd_type_t_SD_TYPE_MXFP4,
    sd_type_t_SD_TYPE_COUNT,
};

#[allow(non_camel_case_types)]
#[cfg_attr(any(not(windows), target_env = "gnu"), repr(u32))] // include windows-gnu
#[cfg_attr(all(windows, not(target_env = "gnu")), repr(i32))] // msvc being *special* again
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum WeightType {
    F32 = sd_type_t_SD_TYPE_F32,
    F16 = sd_type_t_SD_TYPE_F16,
    Q4_0 = sd_type_t_SD_TYPE_Q4_0,
    Q4_1 = sd_type_t_SD_TYPE_Q4_1,
    Q5_0 = sd_type_t_SD_TYPE_Q5_0,
    Q5_1 = sd_type_t_SD_TYPE_Q5_1,
    Q8_0 = sd_type_t_SD_TYPE_Q8_0,
    Q8_1 = sd_type_t_SD_TYPE_Q8_1,
    Q2_K = sd_type_t_SD_TYPE_Q2_K,
    Q3_K = sd_type_t_SD_TYPE_Q3_K,
    Q4_K = sd_type_t_SD_TYPE_Q4_K,
    Q5_K = sd_type_t_SD_TYPE_Q5_K,
    Q6_K = sd_type_t_SD_TYPE_Q6_K,
    Q8_K = sd_type_t_SD_TYPE_Q8_K,
    IQ2_XXS = sd_type_t_SD_TYPE_IQ2_XXS,
    IQ2_XS = sd_type_t_SD_TYPE_IQ2_XS,
    IQ3_XXS = sd_type_t_SD_TYPE_IQ3_XXS,
    IQ1_S = sd_type_t_SD_TYPE_IQ1_S,
    IQ4_NL = sd_type_t_SD_TYPE_IQ4_NL,
    IQ3_S = sd_type_t_SD_TYPE_IQ3_S,
    IQ2_S = sd_type_t_SD_TYPE_IQ2_S,
    IQ4_XS = sd_type_t_SD_TYPE_IQ4_XS,
    I8 = sd_type_t_SD_TYPE_I8,
    I16 = sd_type_t_SD_TYPE_I16,
    I32 = sd_type_t_SD_TYPE_I32,
    I64 = sd_type_t_SD_TYPE_I64,
    F64 = sd_type_t_SD_TYPE_F64,
    IQ1_M = sd_type_t_SD_TYPE_IQ1_M,
    BF16 = sd_type_t_SD_TYPE_BF16,
    TQ1_0 = sd_type_t_SD_TYPE_TQ1_0,
    TQ2_0 = sd_type_t_SD_TYPE_TQ2_0,
    MXFP4 = sd_type_t_SD_TYPE_MXFP4,
    Unknown = sd_type_t_SD_TYPE_COUNT,
}

impl From<WeightType> for sd_type_t {
    fn from(value: WeightType) -> Self {
        value as Self
    }
}

impl Default for WeightType {
    fn default() -> Self {
        Self::Unknown
    }
}
