mod candle_diffusion_service;
mod candle_transformers_service;
mod ggml_diffusion_service;
mod ggml_llama_service;
mod ggml_whisper_service;
mod onnx_service;
mod runtime_service;

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::application::dtos as dto;
use crate::domain::runtime::CoreError;

pub(crate) use candle_diffusion_service::CandleDiffusionService;
pub(crate) use candle_transformers_service::CandleTransformersService;
pub(crate) use ggml_diffusion_service::GgmlDiffusionService;
pub(crate) use ggml_llama_service::GgmlLlamaService;
pub(crate) use ggml_whisper_service::GgmlWhisperService;
pub(crate) use onnx_service::OnnxService;
pub use runtime_service::RuntimeApplication;

pub(crate) type LoadedService<T> = Arc<RwLock<Option<T>>>;

#[derive(Debug)]
pub enum RuntimeApplicationError {
    Runtime(CoreError),
}

pub(crate) fn empty_slot<T>() -> LoadedService<T> {
    Arc::new(RwLock::new(None))
}

impl std::fmt::Display for RuntimeApplicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Runtime(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RuntimeApplicationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Runtime(error) => Some(error),
        }
    }
}

impl From<CoreError> for RuntimeApplicationError {
    fn from(value: CoreError) -> Self {
        Self::Runtime(value)
    }
}

pub(crate) fn model_status(backend: &str, status: &str) -> dto::ModelStatus {
    dto::ModelStatus { backend: backend.to_owned(), status: status.to_owned() }
}

pub(crate) async fn take_loaded<T>(slot: &LoadedService<T>) -> Option<T> {
    let mut guard = slot.write().await;
    guard.take()
}

pub(crate) async fn store_loaded<T>(slot: &LoadedService<T>, value: T) {
    let mut guard = slot.write().await;
    *guard = Some(value);
}

pub(crate) async fn clone_loaded<T: Clone>(
    slot: &LoadedService<T>,
) -> Result<T, RuntimeApplicationError> {
    let guard = slot.read().await;
    guard.clone().ok_or(CoreError::ModelNotLoaded).map_err(RuntimeApplicationError::Runtime)
}
