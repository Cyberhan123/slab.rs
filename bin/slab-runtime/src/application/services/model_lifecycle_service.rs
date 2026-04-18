use std::path::PathBuf;

use slab_types::runtime::RuntimeModelStatus;
use slab_types::{
    GgmlLlamaLoadConfig, GgmlWhisperLoadConfig, ModelSource, ModelSpec, RuntimeBackendLoadSpec,
};

use super::{BackendKind, RuntimeApplicationError, SharedRuntimeState};

#[derive(Clone)]
pub(crate) struct ModelLifecycleService {
    state: SharedRuntimeState,
}

impl ModelLifecycleService {
    pub(crate) fn new(state: SharedRuntimeState) -> Self {
        Self { state }
    }

    pub(crate) async fn load_model_for_backend(
        &self,
        backend: BackendKind,
        load_spec: RuntimeBackendLoadSpec,
    ) -> Result<RuntimeModelStatus, RuntimeApplicationError> {
        let mut state = self.state.write().await;
        state.ensure_enabled(backend)?;

        let spec = build_model_spec(backend, &load_spec);
        let session = state
            .execution
            .session_for_backend(spec, backend.canonical_id())
            .map_err(RuntimeApplicationError::Runtime)?;
        session.load().await.map_err(RuntimeApplicationError::Runtime)?;
        state.sessions.insert(backend, session);

        Ok(model_status(backend, "loaded"))
    }

    pub(crate) async fn unload_model_for_backend(
        &self,
        backend: BackendKind,
    ) -> Result<RuntimeModelStatus, RuntimeApplicationError> {
        let mut state = self.state.write().await;
        state.ensure_enabled(backend)?;

        let session = state
            .sessions
            .remove(&backend)
            .ok_or(slab_runtime_core::CoreError::ModelNotLoaded)
            .map_err(RuntimeApplicationError::Runtime)?;
        session.unload().await.map_err(RuntimeApplicationError::Runtime)?;

        Ok(model_status(backend, "unloaded"))
    }
}

fn build_model_spec(backend: BackendKind, load_spec: &RuntimeBackendLoadSpec) -> ModelSpec {
    let model_path = match load_spec {
        RuntimeBackendLoadSpec::GgmlLlama(GgmlLlamaLoadConfig { model_path, .. })
        | RuntimeBackendLoadSpec::GgmlWhisper(GgmlWhisperLoadConfig { model_path, .. }) => {
            model_path.clone()
        }
        RuntimeBackendLoadSpec::GgmlDiffusion(config) => config.model_path.clone(),
        other => other.to_legacy_spec().model_path,
    };
    let mut spec = ModelSpec::new(
        backend.family(),
        backend.capability(),
        ModelSource::LocalPath { path: model_path },
    );

    match backend {
        BackendKind::Llama => {
            if let RuntimeBackendLoadSpec::GgmlLlama(load_config) = load_spec {
                spec.load_options
                    .insert("num_workers".to_owned(), serde_json::json!(load_config.num_workers));
                spec.load_options.insert(
                    "context_length".to_owned(),
                    serde_json::json!(load_config.context_length.unwrap_or(0)),
                );
                spec.load_options
                    .insert("flash_attn".to_owned(), serde_json::json!(load_config.flash_attn));
                if let Some(chat_template) = &load_config.chat_template {
                    spec.load_options
                        .insert("chat_template".to_owned(), serde_json::json!(chat_template));
                }
            }
        }
        BackendKind::Whisper => {
            if let RuntimeBackendLoadSpec::GgmlWhisper(load_config) = load_spec {
                spec.load_options
                    .insert("flash_attn".to_owned(), serde_json::json!(load_config.flash_attn));
            }
        }
        BackendKind::Diffusion => {
            if let RuntimeBackendLoadSpec::GgmlDiffusion(load_config) = load_spec {
                insert_opt_path_option(
                    &mut spec,
                    "diffusion_model_path",
                    load_config.diffusion_model_path.as_ref(),
                );
                insert_opt_path_option(&mut spec, "vae_path", load_config.vae_path.as_ref());
                insert_opt_path_option(&mut spec, "taesd_path", load_config.taesd_path.as_ref());
                insert_opt_path_option(&mut spec, "clip_l_path", load_config.clip_l_path.as_ref());
                insert_opt_path_option(&mut spec, "clip_g_path", load_config.clip_g_path.as_ref());
                insert_opt_path_option(&mut spec, "t5xxl_path", load_config.t5xxl_path.as_ref());
                insert_opt_path_option(
                    &mut spec,
                    "clip_vision_path",
                    load_config.clip_vision_path.as_ref(),
                );
                insert_opt_path_option(
                    &mut spec,
                    "control_net_path",
                    load_config.control_net_path.as_ref(),
                );
                spec.load_options
                    .insert("flash_attn".to_owned(), serde_json::json!(load_config.flash_attn));
                spec.load_options
                    .insert("vae_device".to_owned(), serde_json::json!(load_config.vae_device));
                spec.load_options
                    .insert("clip_device".to_owned(), serde_json::json!(load_config.clip_device));
                spec.load_options.insert(
                    "offload_params_to_cpu".to_owned(),
                    serde_json::json!(load_config.offload_params_to_cpu),
                );
                spec.load_options
                    .insert("enable_mmap".to_owned(), serde_json::json!(load_config.enable_mmap));
                if let Some(n_threads) = load_config.n_threads {
                    spec.load_options.insert("n_threads".to_owned(), serde_json::json!(n_threads));
                }
            }
        }
    }

    spec
}

fn insert_opt_path_option(spec: &mut ModelSpec, key: &str, value: Option<&PathBuf>) {
    if let Some(value) = value {
        spec.load_options
            .insert(key.to_owned(), serde_json::json!(value.to_string_lossy().into_owned()));
    }
}

fn model_status(backend: BackendKind, status: &str) -> RuntimeModelStatus {
    RuntimeModelStatus { backend: backend.runtime_backend_id(), status: status.to_owned() }
}
