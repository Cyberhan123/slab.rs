use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Typed `model.load` payload for the `ggml.llama` backend.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct GgmlLlamaLoadConfig {
    pub model_path: PathBuf,
    pub num_workers: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_template: Option<String>,
}

/// Typed `model.load` payload for the `ggml.whisper` backend.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct GgmlWhisperLoadConfig {
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
    #[serde(default)]
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
    #[serde(default)]
    pub seed: u64,
}

/// Typed `model.load` payload for the `candle.whisper` backend.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CandleWhisperLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer_path: Option<PathBuf>,
}

/// Typed `model.load` payload for the `candle.diffusion` backend.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CandleDiffusionLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_path: Option<PathBuf>,
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
            Self::GgmlDiffusion(_) => crate::backend::RuntimeBackendId::GgmlDiffusion,
            Self::CandleLlama(_) => crate::backend::RuntimeBackendId::CandleLlama,
            Self::CandleWhisper(_) => crate::backend::RuntimeBackendId::CandleWhisper,
            Self::CandleDiffusion(_) => crate::backend::RuntimeBackendId::CandleDiffusion,
            Self::Onnx(_) => crate::backend::RuntimeBackendId::Onnx,
        }
    }

    pub fn from_legacy(
        backend: crate::backend::RuntimeBackendId,
        spec: crate::runtime::RuntimeModelLoadSpec,
    ) -> Result<Self, crate::error::SlabTypeError> {
        let crate::runtime::RuntimeModelLoadSpec {
            model_path,
            num_workers,
            context_length,
            chat_template,
            diffusion,
        } = spec;

        Ok(match backend {
            crate::backend::RuntimeBackendId::GgmlLlama => Self::GgmlLlama(GgmlLlamaLoadConfig {
                model_path,
                num_workers: usize::try_from(num_workers).map_err(|error| {
                    crate::error::SlabTypeError::Validation {
                        path: "num_workers".to_owned(),
                        message: error.to_string(),
                    }
                })?,
                context_length,
                chat_template,
            }),
            crate::backend::RuntimeBackendId::GgmlWhisper => {
                Self::GgmlWhisper(GgmlWhisperLoadConfig { model_path })
            }
            crate::backend::RuntimeBackendId::GgmlDiffusion => {
                let diffusion = diffusion.unwrap_or_default();
                Self::GgmlDiffusion(Box::new(GgmlDiffusionLoadConfig {
                    model_path,
                    diffusion_model_path: diffusion.diffusion_model_path,
                    vae_path: diffusion.vae_path,
                    taesd_path: diffusion.taesd_path,
                    clip_l_path: diffusion.clip_l_path,
                    clip_g_path: diffusion.clip_g_path,
                    t5xxl_path: diffusion.t5xxl_path,
                    clip_vision_path: None,
                    control_net_path: None,
                    flash_attn: diffusion.flash_attn,
                    vae_device: (!diffusion.vae_device.is_empty()).then_some(diffusion.vae_device),
                    clip_device: (!diffusion.clip_device.is_empty())
                        .then_some(diffusion.clip_device),
                    offload_params_to_cpu: diffusion.offload_params_to_cpu,
                    enable_mmap: false,
                    n_threads: None,
                }))
            }
            crate::backend::RuntimeBackendId::CandleLlama => {
                Self::CandleLlama(CandleLlamaLoadConfig {
                    model_path,
                    tokenizer_path: None,
                    seed: 0,
                })
            }
            crate::backend::RuntimeBackendId::CandleWhisper => {
                Self::CandleWhisper(CandleWhisperLoadConfig { model_path, tokenizer_path: None })
            }
            crate::backend::RuntimeBackendId::CandleDiffusion => {
                let diffusion = diffusion.unwrap_or_default();
                Self::CandleDiffusion(CandleDiffusionLoadConfig {
                    model_path,
                    vae_path: diffusion.vae_path,
                    sd_version: default_candle_sd_version(),
                })
            }
            crate::backend::RuntimeBackendId::Onnx => Self::Onnx(OnnxLoadConfig {
                model_path,
                execution_providers: default_execution_providers(),
                intra_op_num_threads: None,
                inter_op_num_threads: None,
            }),
        })
    }

    pub fn to_legacy_spec(&self) -> crate::runtime::RuntimeModelLoadSpec {
        match self {
            Self::GgmlLlama(config) => crate::runtime::RuntimeModelLoadSpec {
                model_path: config.model_path.clone(),
                num_workers: u32::try_from(config.num_workers).unwrap_or(u32::MAX),
                context_length: config.context_length,
                chat_template: config.chat_template.clone(),
                diffusion: None,
            },
            Self::GgmlWhisper(config) => crate::runtime::RuntimeModelLoadSpec {
                model_path: config.model_path.clone(),
                ..Default::default()
            },
            Self::GgmlDiffusion(config) => crate::runtime::RuntimeModelLoadSpec {
                model_path: config.model_path.clone(),
                diffusion: Some(crate::runtime::DiffusionLoadOptions {
                    diffusion_model_path: config.diffusion_model_path.clone(),
                    vae_path: config.vae_path.clone(),
                    taesd_path: config.taesd_path.clone(),
                    lora_model_dir: None,
                    clip_l_path: config.clip_l_path.clone(),
                    clip_g_path: config.clip_g_path.clone(),
                    t5xxl_path: config.t5xxl_path.clone(),
                    flash_attn: config.flash_attn,
                    vae_device: config.vae_device.clone().unwrap_or_default(),
                    clip_device: config.clip_device.clone().unwrap_or_default(),
                    offload_params_to_cpu: config.offload_params_to_cpu,
                }),
                ..Default::default()
            },
            Self::CandleLlama(config) => crate::runtime::RuntimeModelLoadSpec {
                model_path: config.model_path.clone(),
                ..Default::default()
            },
            Self::CandleWhisper(config) => crate::runtime::RuntimeModelLoadSpec {
                model_path: config.model_path.clone(),
                ..Default::default()
            },
            Self::CandleDiffusion(config) => crate::runtime::RuntimeModelLoadSpec {
                model_path: config.model_path.clone(),
                diffusion: Some(crate::runtime::DiffusionLoadOptions {
                    vae_path: config.vae_path.clone(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Self::Onnx(config) => crate::runtime::RuntimeModelLoadSpec {
                model_path: config.model_path.clone(),
                ..Default::default()
            },
        }
    }
}
