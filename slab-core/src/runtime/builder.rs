use std::path::PathBuf;

use tokio::runtime::Handle;

use crate::base::error::CoreError;
use crate::dispatch::{BackendDriverDescriptor, DispatchPlanner, DriverLoadStyle, ModelSourceKind};
use crate::runtime::runtime::Runtime;
use crate::scheduler::backend::admission::{ResourceManager, ResourceManagerConfig};
use crate::scheduler::orchestrator::Orchestrator;
use crate::spec::{Capability, ModelFamily};

#[derive(Debug, Clone)]
pub struct BuiltinDriversConfig {
    pub llama_lib_dir: Option<PathBuf>,
    pub whisper_lib_dir: Option<PathBuf>,
    pub diffusion_lib_dir: Option<PathBuf>,
    pub onnx_enabled: bool,
    pub enable_candle_llama: bool,
    pub enable_candle_whisper: bool,
    pub enable_candle_diffusion: bool,
}

impl Default for BuiltinDriversConfig {
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

#[derive(Debug, Clone)]
pub struct RuntimeBuilder {
    queue_capacity: usize,
    backend_capacity: usize,
    builtin_drivers: BuiltinDriversConfig,
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self {
            queue_capacity: 64,
            backend_capacity: 4,
            builtin_drivers: BuiltinDriversConfig::default(),
        }
    }

    pub fn queue_capacity(mut self, queue_capacity: usize) -> Self {
        self.queue_capacity = queue_capacity;
        self
    }

    pub fn backend_capacity(mut self, backend_capacity: usize) -> Self {
        self.backend_capacity = backend_capacity;
        self
    }

    pub fn builtin_drivers(mut self, builtin_drivers: BuiltinDriversConfig) -> Self {
        self.builtin_drivers = builtin_drivers;
        self
    }

    pub fn build(self) -> Result<Runtime, CoreError> {
        let _ = Handle::try_current().map_err(|err| CoreError::DeploymentFailed {
            driver_id: "runtime".to_owned(),
            message: format!("RuntimeBuilder::build must run inside a Tokio runtime: {err}"),
        })?;

        let worker_count = self.backend_capacity;
        let mut rm = ResourceManager::with_config(ResourceManagerConfig {
            backend_capacity: worker_count,
            ..ResourceManagerConfig::default()
        });

        let mut descriptors = Vec::new();

        #[cfg(feature = "ggml")]
        {
            use std::path::Path;

            use crate::engine::ggml::{
                diffusion::{DiffusionWorker, GGMLDiffusionEngine},
                llama::{spawn_backend_with_engine as spawn_llama, GGMLLlamaEngine},
                whisper::{GGMLWhisperEngine, WhisperWorker},
            };
            use crate::scheduler::backend::runner::spawn_workers;

            let llama_engine = self
                .builtin_drivers
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

            let whisper_engine = self
                .builtin_drivers
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

            let diffusion_engine = self
                .builtin_drivers
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

            rm.register_backend("ggml.llama", move |shared_rx, control_tx| {
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

            rm.register_backend("ggml.whisper", move |shared_rx, control_tx| {
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

            rm.register_backend("ggml.diffusion", move |shared_rx, control_tx| {
                let count = worker_count.max(1);
                let mut worker_engines: Vec<Option<GGMLDiffusionEngine>> = (1..count)
                    .map(|_| diffusion_engine.as_ref().map(|engine| engine.fork_library()))
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
        }

        #[cfg(feature = "onnx")]
        if self.builtin_drivers.onnx_enabled {
            use crate::engine::onnx::backend::OnnxWorker;
            use crate::scheduler::backend::runner::spawn_workers;

            rm.register_backend("onnx", move |shared_rx, control_tx| {
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

        if self.builtin_drivers.enable_candle_llama {
            use crate::engine::candle::llama::spawn_backend_with_engine as spawn_candle_llama;

            rm.register_backend("candle.llama", move |shared_rx, control_tx| {
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

        if self.builtin_drivers.enable_candle_whisper {
            use crate::engine::candle::whisper::spawn_backend as spawn_candle_whisper;

            rm.register_backend("candle.whisper", move |shared_rx, control_tx| {
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

        if self.builtin_drivers.enable_candle_diffusion {
            use crate::engine::candle::diffusion::spawn_backend as spawn_candle_diffusion;

            rm.register_backend("candle.diffusion", move |shared_rx, control_tx| {
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

        let planner = DispatchPlanner::new(descriptors);
        let orchestrator = Orchestrator::start(rm, self.queue_capacity);

        Ok(Runtime::new(
            orchestrator,
            planner,
            self.builtin_drivers,
        ))
    }
}

fn driver_descriptor(
    driver_id: &str,
    family: ModelFamily,
    capability: Capability,
    supports_streaming: bool,
    load_style: DriverLoadStyle,
    priority: i32,
) -> BackendDriverDescriptor {
    BackendDriverDescriptor {
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
