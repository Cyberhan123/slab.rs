use std::path::{Path, PathBuf};

use slab_candle::{CandleBackendConfig, runtime_registrations as candle_runtime_registrations};
use slab_onnx::{OnnxBackendConfig, runtime_registrations as onnx_runtime_registrations};
use slab_runtime_core::api::{
    Capability, CoreError, DriverDescriptor, DriverLoadStyle, ModelFamily, ModelSourceKind,
    RuntimeBackendRegistration,
};
use slab_runtime_core::backend::{spawn_dedicated_workers, spawn_workers};
use slab_runtime_core::engines::ggml::{
    DiffusionWorker, GGMLDiffusionEngine, GGMLLlamaEngine, GGMLWhisperEngine, WhisperWorker,
    spawn_backend_with_engine as spawn_ggml_llama_backend,
};

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

pub fn runtime_registrations(
    config: &RuntimeDriversConfig,
) -> Result<Vec<RuntimeBackendRegistration>, CoreError> {
    let mut registrations = ggml_runtime_registrations(config)?;
    registrations.extend(candle_runtime_registrations(&CandleBackendConfig {
        enable_llama: config.enable_candle_llama,
        enable_whisper: config.enable_candle_whisper,
        enable_diffusion: config.enable_candle_diffusion,
    }));
    registrations.extend(onnx_runtime_registrations(&OnnxBackendConfig {
        enabled: config.onnx_enabled,
    }));
    Ok(registrations)
}

fn ggml_runtime_registrations(
    config: &RuntimeDriversConfig,
) -> Result<Vec<RuntimeBackendRegistration>, CoreError> {
    let llama_engine = config
        .llama_lib_dir
        .as_deref()
        .map(load_llama_engine)
        .transpose()?;
    let whisper_engine = config
        .whisper_lib_dir
        .as_deref()
        .map(load_whisper_engine)
        .transpose()?;
    let diffusion_engine = config
        .diffusion_lib_dir
        .as_deref()
        .map(load_diffusion_engine)
        .transpose()?;

    Ok(vec![
        RuntimeBackendRegistration::new(
            vec![driver_descriptor(
                "ggml.llama",
                "ggml.llama",
                ModelFamily::Llama,
                Capability::TextGeneration,
                true,
                DriverLoadStyle::DynamicLibraryThenModel,
                20,
            )],
            move |resource_manager, _worker_count| {
                resource_manager.register_backend("ggml.llama", move |shared_rx, control_tx| {
                    spawn_ggml_llama_backend(shared_rx, control_tx, llama_engine);
                });
                Ok(())
            },
        ),
        RuntimeBackendRegistration::new(
            vec![driver_descriptor(
                "ggml.whisper",
                "ggml.whisper",
                ModelFamily::Whisper,
                Capability::AudioTranscription,
                false,
                DriverLoadStyle::DynamicLibraryThenModel,
                20,
            )],
            move |resource_manager, worker_count| {
                resource_manager.register_backend("ggml.whisper", move |shared_rx, control_tx| {
                    let count = worker_count.max(1);
                    let mut worker_engines: Vec<Option<GGMLWhisperEngine>> = (1..count)
                        .map(|_| whisper_engine.as_ref().map(|engine| engine.fork_library()))
                        .collect();
                    worker_engines.insert(0, whisper_engine);
                    let mut worker_engines = worker_engines.into_iter();
                    spawn_workers(shared_rx, control_tx, count, move |worker_id, bc_tx| {
                        let worker_engine = worker_engines.next().unwrap_or(None);
                        WhisperWorker::new(worker_engine, bc_tx, worker_id)
                    });
                });
                Ok(())
            },
        ),
        RuntimeBackendRegistration::new(
            vec![driver_descriptor(
                "ggml.diffusion",
                "ggml.diffusion",
                ModelFamily::Diffusion,
                Capability::ImageGeneration,
                false,
                DriverLoadStyle::DynamicLibraryThenModel,
                20,
            )],
            move |resource_manager, worker_count| {
                resource_manager.register_backend(
                    "ggml.diffusion",
                    move |shared_rx, control_tx| {
                        let count = worker_count.max(1);
                        let mut worker_engines: Vec<Option<GGMLDiffusionEngine>> = (1..count)
                            .map(|_| diffusion_engine.as_ref().map(|engine| engine.fork_library()))
                            .collect();
                        worker_engines.insert(0, diffusion_engine);
                        let mut worker_engines = worker_engines.into_iter();
                        spawn_dedicated_workers(
                            shared_rx,
                            control_tx,
                            count,
                            move |worker_id, bc_tx| {
                                let worker_engine = worker_engines.next().unwrap_or(None);
                                DiffusionWorker::new(worker_engine, bc_tx, worker_id)
                            },
                        );
                    },
                );
                Ok(())
            },
        ),
    ])
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

fn load_llama_engine(path: &Path) -> Result<std::sync::Arc<GGMLLlamaEngine>, CoreError> {
    GGMLLlamaEngine::from_path(path).map_err(|err| CoreError::LibraryLoadFailed {
        backend: "ggml.llama".to_owned(),
        message: err.to_string(),
    })
}

fn load_whisper_engine(path: &Path) -> Result<GGMLWhisperEngine, CoreError> {
    GGMLWhisperEngine::from_path(path).map_err(|err| CoreError::LibraryLoadFailed {
        backend: "ggml.whisper".to_owned(),
        message: err.to_string(),
    })
}

fn load_diffusion_engine(path: &Path) -> Result<GGMLDiffusionEngine, CoreError> {
    GGMLDiffusionEngine::from_path(path).map_err(|err| CoreError::LibraryLoadFailed {
        backend: "ggml.diffusion".to_owned(),
        message: err.to_string(),
    })
}
