use std::path::PathBuf;

use slab_runtime_backend_candle::{
    CandleBackendConfig, descriptors as candle_descriptors, register as register_candle,
};
use slab_runtime_backend_ggml::{
    GgmlBackendConfig, descriptors as ggml_descriptors, register as register_ggml,
};
use slab_runtime_backend_onnx::{
    OnnxBackendConfig, descriptors as onnx_descriptors, register as register_onnx,
};
use slab_runtime_core::backend::ResourceManager;
use slab_runtime_core::CoreError;
use slab_types::DriverDescriptor;

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

pub fn descriptors(config: &RuntimeDriversConfig) -> Vec<DriverDescriptor> {
    let mut descriptors = ggml_descriptors(&GgmlBackendConfig {
        llama_lib_dir: config.llama_lib_dir.clone(),
        whisper_lib_dir: config.whisper_lib_dir.clone(),
        diffusion_lib_dir: config.diffusion_lib_dir.clone(),
    });
    descriptors.extend(candle_descriptors(&CandleBackendConfig {
        enable_llama: config.enable_candle_llama,
        enable_whisper: config.enable_candle_whisper,
        enable_diffusion: config.enable_candle_diffusion,
    }));
    descriptors.extend(onnx_descriptors(&OnnxBackendConfig {
        enabled: config.onnx_enabled,
    }));
    descriptors
}

pub fn register_backends(
    config: &RuntimeDriversConfig,
    resource_manager: &mut ResourceManager,
    worker_count: usize,
) -> Result<(), CoreError> {
    register_ggml(
        &GgmlBackendConfig {
            llama_lib_dir: config.llama_lib_dir.clone(),
            whisper_lib_dir: config.whisper_lib_dir.clone(),
            diffusion_lib_dir: config.diffusion_lib_dir.clone(),
        },
        resource_manager,
        worker_count,
    )?;
    register_candle(
        &CandleBackendConfig {
            enable_llama: config.enable_candle_llama,
            enable_whisper: config.enable_candle_whisper,
            enable_diffusion: config.enable_candle_diffusion,
        },
        resource_manager,
        worker_count,
    )?;
    register_onnx(
        &OnnxBackendConfig {
            enabled: config.onnx_enabled,
        },
        resource_manager,
        worker_count,
    )?;
    Ok(())
}
