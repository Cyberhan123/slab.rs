use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use slab_core::api::{
    Capability, CoreError, DriversConfig, ModelFamily, ModelSource, ModelSpec, Pipeline, Runtime,
    RuntimeBuilder,
};
use slab_proto::{convert, slab::ipc::v1 as pb};
use slab_types::backend::RuntimeBackendId;
use slab_types::runtime::{RuntimeModelLoadSpec, RuntimeModelStatus};
use tokio::sync::RwLock;
use tonic::Status;
use tracing::instrument;

use super::EnabledBackends;

mod diffusion;
mod llama;
mod whisper;

#[derive(Clone)]
pub struct GrpcServiceImpl {
    state: Arc<RwLock<RuntimeState>>,
}

#[derive(Debug)]
struct RuntimeState {
    runtime: Runtime,
    drivers: DriversConfig,
    enabled_backends: EnabledBackends,
    pipelines: HashMap<BackendKind, Pipeline>,
}

#[derive(Clone, Copy, Debug)]
struct BackendDisabled(BackendKind);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum BackendKind {
    Llama,
    Whisper,
    Diffusion,
}

const BACKEND_ORDER: [BackendKind; 3] =
    [BackendKind::Llama, BackendKind::Whisper, BackendKind::Diffusion];

impl GrpcServiceImpl {
    pub fn new(
        runtime: Runtime,
        drivers: DriversConfig,
        enabled_backends: EnabledBackends,
    ) -> Self {
        Self {
            state: Arc::new(RwLock::new(RuntimeState {
                runtime,
                drivers,
                enabled_backends,
                pipelines: HashMap::new(),
            })),
        }
    }

    pub(super) async fn pipeline_for_backend(
        &self,
        backend: BackendKind,
    ) -> Result<Pipeline, Status> {
        let state = self.state.read().await;
        state.ensure_enabled(backend).map_err(Status::from)?;
        state
            .pipelines
            .get(&backend)
            .cloned()
            .ok_or_else(|| runtime_to_status(CoreError::ModelNotLoaded))
    }

    #[instrument(skip_all, fields(backend = backend.canonical_id()))]
    pub(super) async fn load_model_for_backend(
        &self,
        backend: BackendKind,
        request: pb::ModelLoadRequest,
    ) -> Result<pb::ModelStatusResponse, Status> {
        let load_spec = convert::decode_model_load_request(&request).map_err(proto_to_status)?;

        let mut state = self.state.write().await;
        state.ensure_enabled(backend).map_err(Status::from)?;

        let spec = build_model_spec(backend, &load_spec);
        let pipeline = state.runtime.pipeline(spec).map_err(runtime_to_status)?;
        pipeline.load().await.map_err(runtime_to_status)?;
        state.pipelines.insert(backend, pipeline);

        Ok(model_status_response(backend, "loaded"))
    }

    #[instrument(skip_all, fields(backend = backend.canonical_id()))]
    pub(super) async fn unload_model_for_backend(
        &self,
        backend: BackendKind,
    ) -> Result<pb::ModelStatusResponse, Status> {
        let mut state = self.state.write().await;
        state.ensure_enabled(backend).map_err(Status::from)?;

        let pipeline = state
            .pipelines
            .remove(&backend)
            .ok_or_else(|| runtime_to_status(CoreError::ModelNotLoaded))?;
        pipeline.unload().await.map_err(runtime_to_status)?;

        Ok(model_status_response(backend, "unloaded"))
    }

    #[instrument(skip_all, fields(backend = backend.canonical_id(), lib_path = %request.lib_path, model_path = %request.model_path))]
    pub(super) async fn reload_library_for_backend(
        &self,
        backend: BackendKind,
        request: pb::ReloadLibraryRequest,
    ) -> Result<pb::ModelStatusResponse, Status> {
        let reload_spec =
            convert::decode_reload_library_request(&request).map_err(proto_to_status)?;

        let mut state = self.state.write().await;
        state.ensure_enabled(backend).map_err(Status::from)?;

        let mut drivers = state.drivers.clone();
        backend.set_library_path(&mut drivers, reload_spec.lib_path.clone());

        let new_runtime =
            RuntimeBuilder::new().drivers(drivers.clone()).build().map_err(runtime_to_status)?;

        let mut specs: HashMap<BackendKind, ModelSpec> = state
            .pipelines
            .iter()
            .map(|(kind, pipeline)| (*kind, pipeline.model().clone()))
            .collect();

        let target_spec = match specs.remove(&backend) {
            Some(mut spec) => {
                update_model_spec_from_load_spec(backend, &mut spec, &reload_spec.load);
                spec
            }
            None => build_model_spec(backend, &reload_spec.load),
        };
        specs.insert(backend, target_spec);

        let pipelines = load_pipelines(&new_runtime, &specs).await?;

        state.runtime = new_runtime;
        state.drivers = drivers;
        state.pipelines = pipelines;

        Ok(model_status_response(backend, "loaded"))
    }
}

impl RuntimeState {
    fn ensure_enabled(&self, backend: BackendKind) -> Result<(), BackendDisabled> {
        if backend.is_enabled(&self.enabled_backends) {
            Ok(())
        } else {
            Err(BackendDisabled(backend))
        }
    }
}

impl From<BackendDisabled> for Status {
    fn from(value: BackendDisabled) -> Self {
        Status::unavailable(format!("{} backend is disabled", value.0.canonical_id()))
    }
}

impl BackendKind {
    fn runtime_backend_id(self) -> RuntimeBackendId {
        match self {
            Self::Llama => RuntimeBackendId::GgmlLlama,
            Self::Whisper => RuntimeBackendId::GgmlWhisper,
            Self::Diffusion => RuntimeBackendId::GgmlDiffusion,
        }
    }

    pub(super) fn canonical_id(self) -> &'static str {
        self.runtime_backend_id().canonical_id()
    }

    fn driver_id(self) -> &'static str {
        self.canonical_id()
    }

    fn family(self) -> ModelFamily {
        match self {
            Self::Llama => ModelFamily::Llama,
            Self::Whisper => ModelFamily::Whisper,
            Self::Diffusion => ModelFamily::Diffusion,
        }
    }

    fn capability(self) -> Capability {
        match self {
            Self::Llama => Capability::TextGeneration,
            Self::Whisper => Capability::AudioTranscription,
            Self::Diffusion => Capability::ImageGeneration,
        }
    }

    fn is_enabled(self, enabled: &EnabledBackends) -> bool {
        match self {
            Self::Llama => enabled.llama,
            Self::Whisper => enabled.whisper,
            Self::Diffusion => enabled.diffusion,
        }
    }

    fn set_library_path(self, drivers: &mut DriversConfig, path: PathBuf) {
        match self {
            Self::Llama => drivers.llama_lib_dir = Some(path),
            Self::Whisper => drivers.whisper_lib_dir = Some(path),
            Self::Diffusion => drivers.diffusion_lib_dir = Some(path),
        }
    }
}

async fn load_pipelines(
    runtime: &Runtime,
    specs: &HashMap<BackendKind, ModelSpec>,
) -> Result<HashMap<BackendKind, Pipeline>, Status> {
    let mut pipelines = HashMap::new();

    for backend in BACKEND_ORDER {
        let Some(spec) = specs.get(&backend).cloned() else {
            continue;
        };

        let pipeline = runtime.pipeline(spec).map_err(runtime_to_status)?;
        pipeline.load().await.map_err(runtime_to_status)?;
        pipelines.insert(backend, pipeline);
    }

    Ok(pipelines)
}

fn build_model_spec(backend: BackendKind, load_spec: &RuntimeModelLoadSpec) -> ModelSpec {
    let mut spec = ModelSpec::new(
        backend.family(),
        backend.capability(),
        ModelSource::LocalPath { path: load_spec.model_path.clone() },
    );

    spec.driver_hints.prefer_drivers.push(backend.driver_id().to_owned());

    match backend {
        BackendKind::Llama => {
            spec.load_options
                .insert("num_workers".to_owned(), serde_json::json!(load_spec.num_workers));
            spec.load_options.insert(
                "context_length".to_owned(),
                serde_json::json!(load_spec.context_length.unwrap_or(0)),
            );
        }
        BackendKind::Diffusion => {
            if let Some(diffusion) = load_spec.diffusion.as_ref() {
                insert_opt_path_option(
                    &mut spec,
                    "diffusion_model_path",
                    diffusion.diffusion_model_path.as_ref(),
                );
                insert_opt_path_option(&mut spec, "vae_path", diffusion.vae_path.as_ref());
                insert_opt_path_option(&mut spec, "taesd_path", diffusion.taesd_path.as_ref());
                insert_opt_path_option(
                    &mut spec,
                    "lora_model_dir",
                    diffusion.lora_model_dir.as_ref(),
                );
                insert_opt_path_option(&mut spec, "clip_l_path", diffusion.clip_l_path.as_ref());
                insert_opt_path_option(&mut spec, "clip_g_path", diffusion.clip_g_path.as_ref());
                insert_opt_path_option(&mut spec, "t5xxl_path", diffusion.t5xxl_path.as_ref());
                spec.load_options
                    .insert("flash_attn".to_owned(), serde_json::json!(diffusion.flash_attn));
                spec.load_options
                    .insert("vae_device".to_owned(), serde_json::json!(diffusion.vae_device));
                spec.load_options
                    .insert("clip_device".to_owned(), serde_json::json!(diffusion.clip_device));
                spec.load_options.insert(
                    "offload_params_to_cpu".to_owned(),
                    serde_json::json!(diffusion.offload_params_to_cpu),
                );
            }
        }
        BackendKind::Whisper => {}
    }

    spec
}

fn update_model_spec_from_load_spec(
    backend: BackendKind,
    spec: &mut ModelSpec,
    load_spec: &RuntimeModelLoadSpec,
) {
    update_model_source_primary_path(&mut spec.source, load_spec.model_path.clone());
    spec.driver_hints.prefer_drivers = vec![backend.driver_id().to_owned()];
    spec.driver_hints.avoid_drivers.clear();
    spec.driver_hints.require_streaming = false;

    match backend {
        BackendKind::Llama => {
            spec.load_options
                .insert("num_workers".to_owned(), serde_json::json!(load_spec.num_workers));
            spec.load_options.insert(
                "context_length".to_owned(),
                serde_json::json!(load_spec.context_length.unwrap_or(0)),
            );
        }
        BackendKind::Diffusion => {
            if let Some(diffusion) = load_spec.diffusion.as_ref() {
                replace_opt_path_option(
                    spec,
                    "diffusion_model_path",
                    diffusion.diffusion_model_path.as_ref(),
                );
                replace_opt_path_option(spec, "vae_path", diffusion.vae_path.as_ref());
                replace_opt_path_option(spec, "taesd_path", diffusion.taesd_path.as_ref());
                replace_opt_path_option(spec, "lora_model_dir", diffusion.lora_model_dir.as_ref());
                replace_opt_path_option(spec, "clip_l_path", diffusion.clip_l_path.as_ref());
                replace_opt_path_option(spec, "clip_g_path", diffusion.clip_g_path.as_ref());
                replace_opt_path_option(spec, "t5xxl_path", diffusion.t5xxl_path.as_ref());
                spec.load_options
                    .insert("flash_attn".to_owned(), serde_json::json!(diffusion.flash_attn));
                spec.load_options
                    .insert("vae_device".to_owned(), serde_json::json!(diffusion.vae_device));
                spec.load_options
                    .insert("clip_device".to_owned(), serde_json::json!(diffusion.clip_device));
                spec.load_options.insert(
                    "offload_params_to_cpu".to_owned(),
                    serde_json::json!(diffusion.offload_params_to_cpu),
                );
            }
        }
        BackendKind::Whisper => {}
    }
}

fn update_model_source_primary_path(source: &mut ModelSource, path: PathBuf) {
    match source {
        ModelSource::LocalPath { path: current } => *current = path,
        ModelSource::LocalArtifacts { files } | ModelSource::HuggingFace { files, .. } => {
            files.insert("model".to_owned(), path);
        }
        // `ModelSource` is `#[non_exhaustive]`; future variants that carry no
        // embedded file map cannot have their primary path updated here. Log a
        // warning so unexpected sources don't fail silently.
        _ => {
            tracing::warn!(
                "update_model_source_primary_path: unrecognised ModelSource variant; \
                 primary path was not updated"
            );
        }
    }
}

fn insert_opt_path_option(spec: &mut ModelSpec, key: &str, value: Option<&PathBuf>) {
    if let Some(value) = value {
        spec.load_options
            .insert(key.to_owned(), serde_json::json!(value.to_string_lossy().into_owned()));
    }
}

fn replace_opt_path_option(spec: &mut ModelSpec, key: &str, value: Option<&PathBuf>) {
    if let Some(value) = value {
        spec.load_options
            .insert(key.to_owned(), serde_json::json!(value.to_string_lossy().into_owned()));
    }
}

fn model_status_response(backend: BackendKind, status: &str) -> pb::ModelStatusResponse {
    convert::encode_model_status_response(&RuntimeModelStatus {
        backend: backend.runtime_backend_id(),
        status: status.to_owned(),
    })
}

fn format_error_chain(err: &dyn std::error::Error) -> String {
    let mut msg = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        msg.push_str(": ");
        msg.push_str(&cause.to_string());
        source = cause.source();
    }
    msg
}

pub(super) fn runtime_to_status(err: CoreError) -> Status {
    let msg = format_error_chain(&err);
    match err {
        CoreError::NotInitialized | CoreError::ModelNotLoaded => Status::failed_precondition(msg),
        CoreError::QueueFull { .. }
        | CoreError::OrchestratorQueueFull { .. }
        | CoreError::Busy { .. } => Status::resource_exhausted(msg),
        CoreError::TaskNotFound { .. } | CoreError::NoFailedGlobalOperation => {
            Status::not_found(msg)
        }
        CoreError::Timeout | CoreError::BroadcastAckTimeout => Status::deadline_exceeded(msg),
        CoreError::Cancelled => Status::cancelled(msg),
        CoreError::BackendShutdown | CoreError::LibraryLoadFailed { .. } => {
            Status::unavailable(msg)
        }
        CoreError::UnsupportedOperation { .. } | CoreError::UnsupportedCapability { .. } => {
            Status::unimplemented(msg)
        }
        CoreError::InvalidModelSpec { .. } | CoreError::SourceResolveFailed { .. } => {
            Status::invalid_argument(msg)
        }
        CoreError::NoViableDriver { .. } | CoreError::DriverNotRegistered { .. } => {
            Status::failed_precondition(msg)
        }
        CoreError::GlobalStateInconsistent { .. }
        | CoreError::CpuStageFailed { .. }
        | CoreError::GpuStageFailed { .. }
        | CoreError::DeploymentFailed { .. }
        | CoreError::ResultDecodeFailed { .. }
        | CoreError::EngineIo(_)
        | CoreError::GGMLEngine(_)
        | CoreError::OnnxEngine(_)
        | CoreError::CandleEngine(_) => Status::internal(msg),
    }
}

pub(super) fn proto_to_status(err: convert::ProtoConversionError) -> Status {
    Status::invalid_argument(err.to_string())
}

pub(super) fn extract_request_id(metadata: &tonic::metadata::MetadataMap) -> String {
    metadata
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::runtime_to_status;
    use tonic::Code;

    #[test]
    fn engine_errors_map_to_internal_status() {
        let engine_io =
            runtime_to_status(slab_core::api::CoreError::EngineIo("disk offline".into()));
        assert_eq!(engine_io.code(), Code::Internal);
        assert!(engine_io.message().contains("engine I/O error"));

        let ggml =
            runtime_to_status(slab_core::api::CoreError::GGMLEngine("session not found".into()));
        assert_eq!(ggml.code(), Code::Internal);
        assert!(ggml.message().contains("GGML engine error"));
    }

    #[test]
    fn cancelled_error_maps_to_cancelled_status() {
        let status = runtime_to_status(slab_core::api::CoreError::Cancelled);
        assert_eq!(status.code(), Code::Cancelled);
        assert!(status.message().contains("task cancelled"));
    }
}
