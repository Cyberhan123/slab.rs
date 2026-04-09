pub mod diffusion;
pub mod llama;
pub mod whisper;

use slab_runtime_core::CoreError;
use slab_runtime_core::backend::ResourceManager;
use slab_types::{Capability, DriverDescriptor, DriverLoadStyle, ModelFamily, ModelSourceKind};
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

pub fn descriptors(config: &CandleBackendConfig) -> Vec<DriverDescriptor> {
    let mut descriptors = Vec::new();

    if config.enable_llama {
        descriptors.push(driver_descriptor(
            "candle.llama",
            "candle.llama",
            ModelFamily::Llama,
            Capability::TextGeneration,
            true,
            DriverLoadStyle::ModelOnly,
            10,
        ));
    }

    if config.enable_whisper {
        descriptors.push(driver_descriptor(
            "candle.whisper",
            "candle.whisper",
            ModelFamily::Whisper,
            Capability::AudioTranscription,
            false,
            DriverLoadStyle::ModelOnly,
            10,
        ));
    }

    if config.enable_diffusion {
        descriptors.push(driver_descriptor(
            "candle.diffusion",
            "candle.diffusion",
            ModelFamily::Diffusion,
            Capability::ImageGeneration,
            false,
            DriverLoadStyle::ModelOnly,
            10,
        ));
    }

    descriptors
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

fn driver_descriptor(
    driver_id: &str,
    backend_id: &str,
    family: ModelFamily,
    capability: Capability,
    supports_streaming: bool,
    load_style: DriverLoadStyle,
    priority: i32,
) -> DriverDescriptor {
    DriverDescriptor {
        driver_id: driver_id.to_owned(),
        backend_id: backend_id.to_owned(),
        family,
        capability,
        supported_sources: vec![
            ModelSourceKind::LocalPath,
            ModelSourceKind::LocalArtifacts,
            ModelSourceKind::HuggingFace,
        ],
        supports_streaming,
        load_style,
        priority,
    }
}
