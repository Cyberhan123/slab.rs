use crate::error::ParakeetError;
use serde::{Deserialize, Serialize};
use std::ffi::c_int;
use std::path::PathBuf;

/// Stable Rust-native context parameters shared across the runtime chain.
///
/// Parakeet's context params are a strict subset of whisper's: only `use_gpu` and
/// `gpu_device` (no `flash_attn`, no DTW).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_gpu: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_device: Option<c_int>,
}

impl Default for ContextParams {
    fn default() -> Self {
        // Mirror the whisper.cpp default: GPU on, device 0.
        Self { model_path: None, use_gpu: Some(true), gpu_device: None }
    }
}

impl ContextParams {
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self { model_path: Some(model_path.into()), ..Self::default() }
    }
}

/// Inner FFI-mirroring params. Parakeet's `parakeet_context_params` holds no owned
/// buffers, so (unlike whisper) there is no `sync_backing` step.
#[derive(Debug, Clone)]
pub(crate) struct InnerContextParams {
    cp: slab_parakeet_sys::parakeet_context_params,
}

impl InnerContextParams {
    pub(crate) fn from_canonical(
        lib: &slab_parakeet_sys::ParakeetLib,
        value: &ContextParams,
    ) -> Result<Self, ParakeetError> {
        let mut inner = Self { cp: unsafe { lib.parakeet_context_default_params() } };

        if let Some(use_gpu) = value.use_gpu {
            inner.cp.use_gpu = use_gpu;
        }
        if let Some(gpu_device) = value.gpu_device {
            inner.cp.gpu_device = gpu_device;
        }

        Ok(inner)
    }

    pub(crate) fn into_inner(self) -> slab_parakeet_sys::parakeet_context_params {
        self.cp
    }
}
