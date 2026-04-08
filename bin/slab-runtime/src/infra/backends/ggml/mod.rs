mod internal;

pub mod audio_utils;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use slab_runtime_core::backend::{ResourceManager, spawn_dedicated_workers, spawn_workers};
use slab_runtime_core::CoreError;
use slab_types::{
    Capability, DriverDescriptor, DriverLoadStyle, ModelFamily, ModelSourceKind,
};

use crate::infra::backends::ggml::internal::engine::ggml::diffusion::{
    DiffusionWorker, GGMLDiffusionEngine,
};
use crate::infra::backends::ggml::internal::engine::ggml::llama::{
    GGMLLlamaEngine, spawn_backend_with_engine as spawn_ggml_llama_backend,
};
use crate::infra::backends::ggml::internal::engine::ggml::whisper::{
    GGMLWhisperEngine, WhisperWorker,
};

#[derive(Debug, Clone, Default)]
pub struct GgmlBackendConfig {
    pub llama_lib_dir: Option<PathBuf>,
    pub whisper_lib_dir: Option<PathBuf>,
    pub diffusion_lib_dir: Option<PathBuf>,
}

pub fn descriptors(config: &GgmlBackendConfig) -> Vec<DriverDescriptor> {
    let mut descriptors = Vec::new();

    if config.llama_lib_dir.is_some() {
        descriptors.push(driver_descriptor(
            "ggml.llama",
            "ggml.llama",
            ModelFamily::Llama,
            Capability::TextGeneration,
            true,
            DriverLoadStyle::DynamicLibraryThenModel,
            20,
        ));
    }

    if config.whisper_lib_dir.is_some() {
        descriptors.push(driver_descriptor(
            "ggml.whisper",
            "ggml.whisper",
            ModelFamily::Whisper,
            Capability::AudioTranscription,
            false,
            DriverLoadStyle::DynamicLibraryThenModel,
            20,
        ));
    }

    if config.diffusion_lib_dir.is_some() {
        descriptors.push(driver_descriptor(
            "ggml.diffusion",
            "ggml.diffusion",
            ModelFamily::Diffusion,
            Capability::ImageGeneration,
            false,
            DriverLoadStyle::DynamicLibraryThenModel,
            20,
        ));
    }

    descriptors
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
            let mut worker_engines: Vec<Option<GGMLWhisperEngine>> = (1..count)
                .map(|_| Some(whisper_engine.fork_library()))
                .collect();
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
            let mut worker_engines: Vec<Option<GGMLDiffusionEngine>> = (1..count)
                .map(|_| Some(diffusion_engine.fork_library()))
                .collect();
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

fn load_llama_engine(path: &Path) -> Result<Arc<GGMLLlamaEngine>, CoreError> {
    GGMLLlamaEngine::from_path(path)
}

fn load_whisper_engine(path: &Path) -> Result<GGMLWhisperEngine, CoreError> {
    GGMLWhisperEngine::from_path(path)
}

fn load_diffusion_engine(path: &Path) -> Result<GGMLDiffusionEngine, CoreError> {
    GGMLDiffusionEngine::from_path(path)
}
