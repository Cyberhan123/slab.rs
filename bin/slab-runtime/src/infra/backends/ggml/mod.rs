#[allow(dead_code)]
pub mod audio_utils;
pub mod diffusion;
pub mod llama;
pub mod whisper;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use slab_runtime_core::CoreError;
use slab_runtime_core::backend::{ResourceManager, spawn_dedicated_workers, spawn_workers};
use thiserror::Error;

use crate::infra::backends::ggml::diffusion::{DiffusionWorker, GGMLDiffusionEngine};
use crate::infra::backends::ggml::llama::{
    GGMLLlamaEngine, spawn_backend_with_engine as spawn_ggml_llama_backend,
};
use crate::infra::backends::ggml::whisper::{GGMLWhisperEngine, WhisperWorker};

pub use slab_runtime_core::CoreError as EngineError;

#[derive(Debug, Error)]
pub enum GGMLEngineError {
    #[error("I/O error {0}")]
    Io(#[from] std::io::Error),

    #[error("ggml/whisper/error {0}")]
    Whisper(#[from] whisper::GGMLWhisperEngineError),

    #[error("ggml/llama/error {0}")]
    Llama(#[from] llama::GGMLLlamaEngineError),

    #[error("ggml/diffusion/error {0}")]
    Diffusion(#[from] diffusion::GGMLDiffusionEngineError),
}

macro_rules! impl_ggml_from {
    ($($ty:path),+ $(,)?) => {
        $(
            impl From<$ty> for slab_runtime_core::CoreError {
                fn from(error: $ty) -> Self {
                    slab_runtime_core::CoreError::GGMLEngine(error.to_string())
                }
            }
        )+
    };
}

impl_ggml_from!(
    GGMLEngineError,
    whisper::GGMLWhisperEngineError,
    llama::GGMLLlamaEngineError,
    diffusion::GGMLDiffusionEngineError,
);

#[derive(Debug, Clone, Default)]
pub struct GgmlBackendConfig {
    pub llama_lib_dir: Option<PathBuf>,
    pub whisper_lib_dir: Option<PathBuf>,
    pub diffusion_lib_dir: Option<PathBuf>,
}

pub fn service_ids(config: &GgmlBackendConfig) -> Vec<&'static str> {
    let mut service_ids = Vec::new();

    if config.llama_lib_dir.is_some() {
        service_ids.push("ggml.llama");
    }

    if config.whisper_lib_dir.is_some() {
        service_ids.push("ggml.whisper");
    }

    if config.diffusion_lib_dir.is_some() {
        service_ids.push("ggml.diffusion");
    }

    service_ids
}

pub fn register(
    config: &GgmlBackendConfig,
    resource_manager: &mut ResourceManager,
    worker_count: usize,
) -> Result<(), CoreError> {
    if let Some(path) = config.llama_lib_dir.as_deref() {
        let llama_engine = load_llama_engine(path)?;
        resource_manager.register_backend("ggml.llama", move |shared_rx, control_tx| {
            spawn_ggml_llama_backend(shared_rx, control_tx, Some(Arc::clone(&llama_engine)));
        });
    }

    if let Some(path) = config.whisper_lib_dir.as_deref() {
        let whisper_engine = load_whisper_engine(path)?;
        resource_manager.register_backend("ggml.whisper", move |shared_rx, control_tx| {
            let count = worker_count.max(1);
            let mut worker_engines: Vec<Option<GGMLWhisperEngine>> =
                (1..count).map(|_| Some(whisper_engine.fork_library())).collect();
            worker_engines.insert(0, Some(whisper_engine));
            let mut worker_engines = worker_engines.into_iter();
            spawn_workers(shared_rx, control_tx, count, move |worker_id, bc_tx| {
                let worker_engine = worker_engines.next().unwrap_or(None);
                WhisperWorker::new(worker_engine, bc_tx, worker_id)
            });
        });
    }

    if let Some(path) = config.diffusion_lib_dir.as_deref() {
        let diffusion_engine = load_diffusion_engine(path)?;
        resource_manager.register_backend("ggml.diffusion", move |shared_rx, control_tx| {
            let count = worker_count.max(1);
            let mut worker_engines: Vec<Option<GGMLDiffusionEngine>> =
                (1..count).map(|_| Some(diffusion_engine.fork_library())).collect();
            worker_engines.insert(0, Some(diffusion_engine));
            let mut worker_engines = worker_engines.into_iter();
            spawn_dedicated_workers(shared_rx, control_tx, count, move |worker_id, bc_tx| {
                let worker_engine = worker_engines.next().unwrap_or(None);
                DiffusionWorker::new(worker_engine, bc_tx, worker_id)
            });
        });
    }

    Ok(())
}

fn load_llama_engine(path: &Path) -> Result<Arc<GGMLLlamaEngine>, CoreError> {
    GGMLLlamaEngine::from_path(path)
}

fn load_whisper_engine(path: &Path) -> Result<GGMLWhisperEngine, CoreError> {
    GGMLWhisperEngine::from_path(path)
}

fn load_diffusion_engine(path: &Path) -> Result<GGMLDiffusionEngine, CoreError> {
    GGMLDiffusionEngine::from_path(path)
}
