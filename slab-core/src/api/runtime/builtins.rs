use std::path::PathBuf;

use crate::base::error::CoreError;
use crate::internal::dispatch::{DriverDescriptor, DriverLoadStyle, ModelSourceKind};
use crate::internal::scheduler::backend::admission::ResourceManager;
use crate::model::{Capability, ModelFamily};

#[derive(Debug, Clone)]
pub struct DriversConfig {
    pub llama_lib_dir: Option<PathBuf>,
    pub whisper_lib_dir: Option<PathBuf>,
    pub diffusion_lib_dir: Option<PathBuf>,
    pub onnx_enabled: bool,
    pub enable_candle_llama: bool,
    pub enable_candle_whisper: bool,
    pub enable_candle_diffusion: bool,
}

impl Default for DriversConfig {
    fn default() -> Self {
        Self {
            llama_lib_dir: None,
            whisper_lib_dir: None,
            diffusion_lib_dir: None,
            onnx_enabled: false,
            enable_candle_llama: cfg!(feature = "candle"),
            enable_candle_whisper: cfg!(feature = "candle"),
            enable_candle_diffusion: cfg!(feature = "candle"),
        }
    }
}

pub(crate) fn register_builtin_drivers(
    resource_manager: &mut ResourceManager,
    drivers: &DriversConfig,
    worker_count: usize,
) -> Result<Vec<DriverDescriptor>, CoreError> {
    let mut descriptors = Vec::new();

    #[cfg(feature = "ggml")]
    register_ggml_drivers(resource_manager, drivers, worker_count, &mut descriptors)?;

    #[cfg(feature = "onnx")]
    if drivers.onnx_enabled {
        register_onnx_drivers(resource_manager, worker_count, &mut descriptors);
    }

    if drivers.enable_candle_llama {
        register_candle_llama(resource_manager, &mut descriptors);
    }

    if drivers.enable_candle_whisper {
        register_candle_whisper(resource_manager, worker_count, &mut descriptors);
    }

    if drivers.enable_candle_diffusion {
        register_candle_diffusion(resource_manager, worker_count, &mut descriptors);
    }

    Ok(descriptors)
}

#[cfg(feature = "ggml")]
fn register_ggml_drivers(
    resource_manager: &mut ResourceManager,
    drivers: &DriversConfig,
    worker_count: usize,
    descriptors: &mut Vec<DriverDescriptor>,
) -> Result<(), CoreError> {
    use std::path::Path;

    use crate::internal::engine::ggml::{
        diffusion::{DiffusionWorker, GGMLDiffusionEngine},
        llama::{spawn_backend_with_engine as spawn_llama, GGMLLlamaEngine},
        whisper::{GGMLWhisperEngine, WhisperWorker},
    };
    use crate::internal::scheduler::backend::runner::spawn_workers;

    let llama_engine = drivers
        .llama_lib_dir
        .as_deref()
        .map(|path| {
            GGMLLlamaEngine::from_path(Path::new(path)).map_err(|err| {
                CoreError::LibraryLoadFailed {
                    backend: "ggml.llama".to_owned(),
                    message: err.to_string(),
                }
            })
        })
        .transpose()?;

    let whisper_engine = drivers
        .whisper_lib_dir
        .as_deref()
        .map(|path| {
            GGMLWhisperEngine::from_path(Path::new(path)).map_err(|err| {
                CoreError::LibraryLoadFailed {
                    backend: "ggml.whisper".to_owned(),
                    message: err.to_string(),
                }
            })
        })
        .transpose()?;

    let diffusion_engine = drivers
        .diffusion_lib_dir
        .as_deref()
        .map(|path| {
            GGMLDiffusionEngine::from_path(Path::new(path)).map_err(|err| {
                CoreError::LibraryLoadFailed {
                    backend: "ggml.diffusion".to_owned(),
                    message: err.to_string(),
                }
            })
        })
        .transpose()?;

    resource_manager.register_backend("ggml.llama", move |shared_rx, control_tx| {
        spawn_llama(shared_rx, control_tx, llama_engine);
    });
    descriptors.push(driver_descriptor(
        "ggml.llama",
        ModelFamily::Llama,
        Capability::TextGeneration,
        true,
        DriverLoadStyle::DynamicLibraryThenModel,
        20,
    ));

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
    descriptors.push(driver_descriptor(
        "ggml.whisper",
        ModelFamily::Whisper,
        Capability::AudioTranscription,
        false,
        DriverLoadStyle::DynamicLibraryThenModel,
        20,
    ));

    resource_manager.register_backend("ggml.diffusion", move |shared_rx, control_tx| {
        let count = worker_count.max(1);
        let mut worker_engines: Vec<Option<GGMLDiffusionEngine>> = (1..count)
            .map(|_| {
                diffusion_engine
                    .as_ref()
                    .map(|engine| engine.fork_library())
            })
            .collect();
        worker_engines.insert(0, diffusion_engine);
        let mut worker_engines = worker_engines.into_iter();
        spawn_workers(shared_rx, control_tx, count, move |worker_id, bc_tx| {
            let worker_engine = worker_engines.next().unwrap_or(None);
            DiffusionWorker::new(worker_engine, bc_tx, worker_id)
        });
    });
    descriptors.push(driver_descriptor(
        "ggml.diffusion",
        ModelFamily::Diffusion,
        Capability::ImageGeneration,
        false,
        DriverLoadStyle::DynamicLibraryThenModel,
        20,
    ));

    Ok(())
}

#[cfg(feature = "onnx")]
fn register_onnx_drivers(
    resource_manager: &mut ResourceManager,
    worker_count: usize,
    descriptors: &mut Vec<DriverDescriptor>,
) {
    use crate::internal::engine::onnx::backend::OnnxWorker;
    use crate::internal::scheduler::backend::runner::spawn_workers;

    resource_manager.register_backend("onnx", move |shared_rx, control_tx| {
        let count = worker_count.max(1);
        spawn_workers(shared_rx, control_tx, count, move |worker_id, bc_tx| {
            OnnxWorker::new(bc_tx, worker_id)
        });
    });

    descriptors.push(driver_descriptor(
        "onnx.text",
        ModelFamily::Onnx,
        Capability::TextGeneration,
        false,
        DriverLoadStyle::ModelOnly,
        30,
    ));
    descriptors.push(driver_descriptor(
        "onnx.embedding",
        ModelFamily::Onnx,
        Capability::ImageEmbedding,
        false,
        DriverLoadStyle::ModelOnly,
        10,
    ));
}

fn register_candle_llama(
    resource_manager: &mut ResourceManager,
    descriptors: &mut Vec<DriverDescriptor>,
) {
    use crate::internal::engine::candle::llama::spawn_backend_with_engine as spawn_candle_llama;

    resource_manager.register_backend("candle.llama", move |shared_rx, control_tx| {
        spawn_candle_llama(shared_rx, control_tx, None);
    });
    descriptors.push(driver_descriptor(
        "candle.llama",
        ModelFamily::Llama,
        Capability::TextGeneration,
        true,
        DriverLoadStyle::ModelOnly,
        10,
    ));
}

fn register_candle_whisper(
    resource_manager: &mut ResourceManager,
    worker_count: usize,
    descriptors: &mut Vec<DriverDescriptor>,
) {
    use crate::internal::engine::candle::whisper::spawn_backend as spawn_candle_whisper;

    resource_manager.register_backend("candle.whisper", move |shared_rx, control_tx| {
        spawn_candle_whisper(shared_rx, control_tx, worker_count);
    });
    descriptors.push(driver_descriptor(
        "candle.whisper",
        ModelFamily::Whisper,
        Capability::AudioTranscription,
        false,
        DriverLoadStyle::ModelOnly,
        10,
    ));
}

fn register_candle_diffusion(
    resource_manager: &mut ResourceManager,
    worker_count: usize,
    descriptors: &mut Vec<DriverDescriptor>,
) {
    use crate::internal::engine::candle::diffusion::spawn_backend as spawn_candle_diffusion;

    resource_manager.register_backend("candle.diffusion", move |shared_rx, control_tx| {
        spawn_candle_diffusion(shared_rx, control_tx, worker_count);
    });
    descriptors.push(driver_descriptor(
        "candle.diffusion",
        ModelFamily::Diffusion,
        Capability::ImageGeneration,
        false,
        DriverLoadStyle::ModelOnly,
        10,
    ));
}

fn driver_descriptor(
    driver_id: &str,
    family: ModelFamily,
    capability: Capability,
    supports_streaming: bool,
    load_style: DriverLoadStyle,
    priority: i32,
) -> DriverDescriptor {
    DriverDescriptor {
        driver_id: driver_id.to_owned(),
        backend_id: backend_id_for_driver(driver_id).to_owned(),
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

fn backend_id_for_driver(driver_id: &str) -> &str {
    match driver_id {
        "onnx.text" | "onnx.embedding" => "onnx",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onnx_aliases_share_backend_id() {
        assert_eq!(backend_id_for_driver("onnx.text"), "onnx");
        assert_eq!(backend_id_for_driver("onnx.embedding"), "onnx");
        assert_eq!(backend_id_for_driver("candle.llama"), "candle.llama");
    }

    #[test]
    fn driver_descriptors_keep_common_supported_sources() {
        let descriptor = driver_descriptor(
            "candle.llama",
            ModelFamily::Llama,
            Capability::TextGeneration,
            true,
            DriverLoadStyle::ModelOnly,
            10,
        );

        assert_eq!(
            descriptor.supported_sources,
            vec![
                ModelSourceKind::LocalPath,
                ModelSourceKind::LocalArtifacts,
                ModelSourceKind::HuggingFace,
            ]
        );
    }
}
