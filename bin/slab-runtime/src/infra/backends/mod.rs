use std::path::PathBuf;

use slab_runtime_core::backend::ResourceManager;
use slab_runtime_core::CoreError;
use slab_types::DriverDescriptor;

use crate::infra::config::RuntimeConfig;

#[cfg(feature = "candle")]
pub(crate) mod candle;
#[cfg(feature = "ggml")]
pub(crate) mod ggml;
#[cfg(feature = "onnx")]
pub(crate) mod onnx;

#[derive(Debug, Clone, Default)]
pub struct RuntimeDriversConfig {
    pub llama_lib_dir: Option<PathBuf>,
    pub whisper_lib_dir: Option<PathBuf>,
    pub diffusion_lib_dir: Option<PathBuf>,
    pub onnx_enabled: bool,
    pub enable_candle_llama: bool,
    pub enable_candle_whisper: bool,
    pub enable_candle_diffusion: bool,
}

impl From<&RuntimeConfig> for RuntimeDriversConfig {
    fn from(value: &RuntimeConfig) -> Self {
        Self {
            llama_lib_dir: value.llama_lib_dir.clone(),
            whisper_lib_dir: value.whisper_lib_dir.clone(),
            diffusion_lib_dir: value.diffusion_lib_dir.clone(),
            onnx_enabled: value.onnx_enabled,
            enable_candle_llama: value.enable_candle_llama,
            enable_candle_whisper: value.enable_candle_whisper,
            enable_candle_diffusion: value.enable_candle_diffusion,
        }
    }
}

pub fn descriptors(
    _config: &RuntimeDriversConfig,
) -> Vec<DriverDescriptor> {
    #[allow(unused_mut)]
    let mut descriptors = Vec::new();

    #[cfg(feature = "ggml")]
    descriptors.extend(ggml::descriptors(&ggml::GgmlBackendConfig {
        llama_lib_dir: _config.llama_lib_dir.clone(),
        whisper_lib_dir: _config.whisper_lib_dir.clone(),
        diffusion_lib_dir: _config.diffusion_lib_dir.clone(),
    }));

    #[cfg(feature = "candle")]
    descriptors.extend(candle::descriptors(&candle::CandleBackendConfig {
        enable_llama: _config.enable_candle_llama,
        enable_whisper: _config.enable_candle_whisper,
        enable_diffusion: _config.enable_candle_diffusion,
    }));

    #[cfg(feature = "onnx")]
    descriptors.extend(onnx::descriptors(&onnx::OnnxBackendConfig {
        enabled: _config.onnx_enabled,
    }));

    descriptors
}

pub fn register_backends(
    _config: &RuntimeDriversConfig,
    _resource_manager: &mut ResourceManager,
    _worker_count: usize,
) -> Result<(), CoreError> {
    #[cfg(feature = "ggml")]
    ggml::register(
        &ggml::GgmlBackendConfig {
            llama_lib_dir: _config.llama_lib_dir.clone(),
            whisper_lib_dir: _config.whisper_lib_dir.clone(),
            diffusion_lib_dir: _config.diffusion_lib_dir.clone(),
        },
        _resource_manager,
        _worker_count,
    )?;

    #[cfg(feature = "candle")]
    candle::register(
        &candle::CandleBackendConfig {
            enable_llama: _config.enable_candle_llama,
            enable_whisper: _config.enable_candle_whisper,
            enable_diffusion: _config.enable_candle_diffusion,
        },
        _resource_manager,
        _worker_count,
    )?;

    #[cfg(feature = "onnx")]
    onnx::register(
        &onnx::OnnxBackendConfig {
            enabled: _config.onnx_enabled,
        },
        _resource_manager,
        _worker_count,
    )?;

    Ok(())
}
