use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ContextLengthSpec, GbnfAssetRef, RuntimeDevicePreference, TemplateAssetRef, defaults};

/// Typed `model.load` payload for the `ggml.llama` backend.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct GgmlLlamaLoadConfig {
    pub model_path: PathBuf,
    pub num_workers: usize,
    /// Context window: an explicit token count, or `auto` (resolve at load to
    /// the largest context that fits in GPU VRAM). `None` is treated as `auto`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<ContextLengthSpec>,
    /// Free GPU VRAM (bytes) snapshot taken by the server at dispatch time, used
    /// to size `auto` context. `None` when no VRAM signal is available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_vram_bytes: Option<u64>,
    #[serde(default = "defaults::flash_attn_enabled")]
    pub flash_attn: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gbnf: Option<String>,
    /// Optional multimodal vision/audio projector (mmproj) GGUF. When set, the
    /// runtime loads an mtmd context bound to the text model for image inputs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mmproj_path: Option<PathBuf>,
    /// Scheduler sizing tunables forwarded from the server so `auto` context
    /// sizing uses host policy. `None` fields fall back per-field to
    /// `SchedulerParams::default()` on the runtime side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vram_buffer_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_context_quantum: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_context_fallback: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct GgmlLlamaLoadDefaultsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_template: Option<TemplateAssetRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gbnf: Option<GbnfAssetRef>,
}

/// Typed `model.load` payload for the `ggml.whisper` backend.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct GgmlWhisperLoadConfig {
    pub model_path: PathBuf,
    #[serde(default = "defaults::flash_attn_enabled")]
    pub flash_attn: bool,
}

/// Typed `model.load` payload for the `ggml.parakeet` backend.
///
/// Parakeet's context params only expose `use_gpu` / `gpu_device` (no `flash_attn`),
/// so the load config is a strict subset of whisper's.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct GgmlParakeetLoadConfig {
    pub model_path: PathBuf,
}

/// Typed `model.load` payload for the `ggml.diffusion` backend.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct GgmlDiffusionLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffusion_model_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taesd_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_l_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_g_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t5xxl_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_vision_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_net_path: Option<PathBuf>,
    #[serde(default = "defaults::flash_attn_enabled")]
    pub flash_attn: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_device: Option<String>,
    #[serde(default)]
    pub offload_params_to_cpu: bool,
    #[serde(default)]
    pub enable_mmap: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n_threads: Option<i32>,
}

/// Typed `model.load` payload for the `candle.llama` backend.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CandleLlamaLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<RuntimeDevicePreference>,
    #[serde(default)]
    pub seed: u64,
}

/// Typed `model.load` payload for the `candle.whisper` backend.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CandleWhisperLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<RuntimeDevicePreference>,
}

/// Typed `model.load` payload for the `candle.diffusion` backend.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CandleDiffusionLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<RuntimeDevicePreference>,
    #[serde(default = "default_candle_sd_version")]
    pub sd_version: String,
}

/// Typed `model.load` payload for the `onnx.*` backends.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct OnnxLoadConfig {
    pub model_path: PathBuf,
    #[serde(default = "default_execution_providers")]
    pub execution_providers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intra_op_num_threads: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inter_op_num_threads: Option<usize>,
}

fn default_candle_sd_version() -> String {
    "v2-1".to_owned()
}

fn default_execution_providers() -> Vec<String> {
    vec!["CPU".to_owned()]
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "backend", content = "config", rename_all = "snake_case")]
pub enum RuntimeBackendLoadSpec {
    GgmlLlama(GgmlLlamaLoadConfig),
    GgmlWhisper(GgmlWhisperLoadConfig),
    GgmlParakeet(GgmlParakeetLoadConfig),
    GgmlDiffusion(Box<GgmlDiffusionLoadConfig>),
    CandleLlama(CandleLlamaLoadConfig),
    CandleWhisper(CandleWhisperLoadConfig),
    CandleDiffusion(CandleDiffusionLoadConfig),
    Onnx(OnnxLoadConfig),
}

impl RuntimeBackendLoadSpec {
    pub fn backend(&self) -> crate::backend::RuntimeBackendId {
        match self {
            Self::GgmlLlama(_) => crate::backend::RuntimeBackendId::GgmlLlama,
            Self::GgmlWhisper(_) => crate::backend::RuntimeBackendId::GgmlWhisper,
            Self::GgmlParakeet(_) => crate::backend::RuntimeBackendId::GgmlParakeet,
            Self::GgmlDiffusion(_) => crate::backend::RuntimeBackendId::GgmlDiffusion,
            Self::CandleLlama(_) => crate::backend::RuntimeBackendId::CandleLlama,
            Self::CandleWhisper(_) => crate::backend::RuntimeBackendId::CandleWhisper,
            Self::CandleDiffusion(_) => crate::backend::RuntimeBackendId::CandleDiffusion,
            Self::Onnx(_) => crate::backend::RuntimeBackendId::Onnx,
        }
    }

    pub fn model_path(&self) -> &Path {
        match self {
            Self::GgmlLlama(config) => config.model_path.as_path(),
            Self::GgmlWhisper(config) => config.model_path.as_path(),
            Self::GgmlParakeet(config) => config.model_path.as_path(),
            Self::GgmlDiffusion(config) => config.model_path.as_path(),
            Self::CandleLlama(config) => config.model_path.as_path(),
            Self::CandleWhisper(config) => config.model_path.as_path(),
            Self::CandleDiffusion(config) => config.model_path.as_path(),
            Self::Onnx(config) => config.model_path.as_path(),
        }
    }
}
