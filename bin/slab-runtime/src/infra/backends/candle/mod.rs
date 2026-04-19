pub mod diffusion;
pub mod llama;
pub mod whisper;

use slab_runtime_core::CoreError;
use slab_runtime_core::backend::ResourceManager;
use thiserror::Error;

pub use slab_runtime_core::CoreError as EngineError;

#[derive(Debug, Error)]
pub enum CandleEngineError {
    #[error("candle/llama/error {0}")]
    Llama(#[from] llama::CandleLlamaEngineError),

    #[error("candle/whisper/error {0}")]
    Whisper(#[from] whisper::CandleWhisperEngineError),

    #[error("candle/diffusion/error {0}")]
    Diffusion(#[from] diffusion::CandleDiffusionEngineError),
}

macro_rules! impl_candle_from {
    ($($ty:path),+ $(,)?) => {
        $(
            impl From<$ty> for slab_runtime_core::CoreError {
                fn from(error: $ty) -> Self {
                    slab_runtime_core::CoreError::CandleEngine(error.to_string())
                }
            }
        )+
    };
}

impl_candle_from!(
    CandleEngineError,
    llama::CandleLlamaEngineError,
    whisper::CandleWhisperEngineError,
    diffusion::CandleDiffusionEngineError,
);

#[derive(Debug, Clone, Default)]
pub struct CandleBackendConfig {
    pub enable_llama: bool,
    pub enable_whisper: bool,
    pub enable_diffusion: bool,
}

pub fn service_ids(config: &CandleBackendConfig) -> Vec<&'static str> {
    let mut service_ids = Vec::new();

    if config.enable_llama {
        service_ids.push("candle.llama");
    }

    if config.enable_whisper {
        service_ids.push("candle.whisper");
    }

    if config.enable_diffusion {
        service_ids.push("candle.diffusion");
    }

    service_ids
}

pub fn register(
    config: &CandleBackendConfig,
    resource_manager: &mut ResourceManager,
    worker_count: usize,
) -> Result<(), CoreError> {
    if config.enable_llama {
        resource_manager.register_backend("candle.llama", move |shared_rx, control_tx| {
            llama::spawn_backend_with_engine(shared_rx, control_tx, None);
        });
    }

    if config.enable_whisper {
        resource_manager.register_backend("candle.whisper", move |shared_rx, control_tx| {
            whisper::spawn_backend(shared_rx, control_tx, worker_count);
        });
    }

    if config.enable_diffusion {
        resource_manager.register_backend("candle.diffusion", move |shared_rx, control_tx| {
            diffusion::spawn_backend(shared_rx, control_tx, worker_count);
        });
    }

    Ok(())
}
