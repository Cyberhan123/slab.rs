use std::path::PathBuf;
use std::sync::Arc;

use tokio::runtime::Handle;

use crate::base::error::CoreError;
use crate::internal::scheduler::backend::admission::{ResourceManager, ResourceManagerConfig};
use crate::internal::scheduler::orchestrator::Orchestrator;
use crate::internal::dispatch::{
    DriverDescriptor, DriverLoadStyle, DriverResolver, ModelSourceKind,
};

use super::model::{Capability, ModelFamily, ModelSpec};
use super::pipeline::Pipeline;

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

#[derive(Debug, Clone)]
pub struct RuntimeBuilder {
    queue_capacity: usize,
    backend_capacity: usize,
    drivers: DriversConfig,
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
            drivers: DriversConfig::default(),
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

    pub fn drivers(mut self, drivers: DriversConfig) -> Self {
        self.drivers = drivers;
        self
    }

    pub fn build(self) -> Result<Runtime, CoreError> {
        let _ = Handle::try_current().map_err(|err| CoreError::DeploymentFailed {
            driver_id: "runtime".to_owned(),
            message: format!("RuntimeBuilder::build must run inside a Tokio runtime: {err}"),
        })?;

        let worker_count = self.backend_capacity;
        let mut resource_manager = ResourceManager::with_config(ResourceManagerConfig {
            backend_capacity: worker_count,
            ..ResourceManagerConfig::default()
        });

        let mut descriptors = Vec::new();

        #[cfg(feature = "ggml")]
        {
            use std::path::Path;

            use crate::internal::engine::ggml::{
                diffusion::{DiffusionWorker, GGMLDiffusionEngine},
                llama::{spawn_backend_with_engine as spawn_llama, GGMLLlamaEngine},
                whisper::{GGMLWhisperEngine, WhisperWorker},
            };
            use crate::internal::scheduler::backend::runner::spawn_workers;

            let llama_engine = self
                .drivers
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
                .drivers
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
                .drivers
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
        if self.drivers.onnx_enabled {
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

        if self.drivers.enable_candle_llama {
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

        if self.drivers.enable_candle_whisper {
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

        if self.drivers.enable_candle_diffusion {
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

        let resolver = DriverResolver::new(descriptors);
        let orchestrator = Orchestrator::start(resource_manager, self.queue_capacity);

        Ok(Runtime::new(orchestrator, resolver, self.drivers))
    }
}

#[derive(Clone)]
pub struct Runtime {
    inner: Arc<RuntimeInner>,
}

#[derive(Debug)]
pub(crate) struct RuntimeInner {
    pub orchestrator: Orchestrator,
    pub resolver: DriverResolver,
    pub drivers: DriversConfig,
}

impl Runtime {
    pub(crate) fn new(
        orchestrator: Orchestrator,
        resolver: DriverResolver,
        drivers: DriversConfig,
    ) -> Self {
        Self {
            inner: Arc::new(RuntimeInner {
                orchestrator,
                resolver,
                drivers,
            }),
        }
    }

    pub fn pipeline(&self, spec: ModelSpec) -> Result<Pipeline, CoreError> {
        Pipeline::new(self.clone(), spec)
    }

    pub fn drivers(&self) -> &DriversConfig {
        &self.inner.drivers
    }

    pub(crate) fn orchestrator(&self) -> Orchestrator {
        self.inner.orchestrator.clone()
    }

    pub(crate) fn kernel(&self) -> crate::internal::scheduler::kernel::ExecutionKernel {
        crate::internal::scheduler::kernel::ExecutionKernel::new(self.inner.orchestrator.clone())
    }

    pub(crate) fn resolver(&self) -> &DriverResolver {
        &self.inner.resolver
    }
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("drivers", &self.inner.drivers)
            .field("driver_count", &self.inner.resolver.descriptors().len())
            .finish()
    }
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
