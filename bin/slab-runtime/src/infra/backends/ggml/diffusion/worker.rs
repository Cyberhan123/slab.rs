//! Backend worker for `ggml.diffusion`.
//!
//! Defines [`DiffusionWorker`] logic for runtime-managed worker loops.
//!
//! # Supported ops
//!
//! | Op string           | Event variant    | Description                                       |
//! |---------------------|------------------|---------------------------------------------------|
//! | `"model.load"`      | `LoadModel`      | Load a model from the engine.                     |
//! | `"model.unload"`    | `UnloadModel`    | Drop the model handle; call model.load to restore. |
//! | `"inference.image"` | `InferenceImage` | Image generation from typed diffusion params.     |
//! ### `model.load` input payload
//! Uses a typed runtime-owned `GgmlDiffusionLoadConfig` payload inside `slab-runtime`.

use std::time::Instant;

use super::contract::{GgmlDiffusionLoadConfig, ImageGenerationRequest, ImageGenerationResponse};
use super::error::GGMLDiffusionWorkerError;
use super::engine::GGMLDiffusionEngine;
use slab_runtime_core::Payload;
use slab_runtime_core::backend::{BroadcastSeq, ControlOpId, Input, PeerControlBus, Typed};
use slab_runtime_macros::backend_handler;

// ── Configurations ────────────────────────────────────────────────────────────

// ── Worker ────────────────────────────────────────────────────────────────────

/// A single diffusion backend worker.
///
/// Each worker **owns** its engine (library handle + model context).  There is
/// no shared mutable state between workers, so no `Mutex` is needed on the
/// context.  When `num_workers > 1` multiple workers are spawned; each
/// worker owns an independent engine forked from the same library handle and
/// manages its own model context independently.
///
/// Workers listen on both the shared `mpsc` ingress queue (competitive –
/// only one worker processes each request) and a `broadcast` channel
/// (fan-out – every worker receives management commands such as `Unload`).
pub struct DiffusionWorker {
    /// - `None` → engine not initialized.
    /// - `Some(e)` where `e.ctx` is None → engine loaded, no model.
    /// - `Some(e)` where `e.ctx` is Some → engine + model loaded.
    engine: Option<GGMLDiffusionEngine>,
    /// Peer synchronization emitter shared among workers.
    peer_bus: PeerControlBus,
    last_model_config: Option<Payload>,
}

#[backend_handler(peer_bus = peer_bus)]
impl DiffusionWorker {
    pub fn new(engine: Option<GGMLDiffusionEngine>, peer_bus: PeerControlBus) -> Self {
        Self { engine, peer_bus, last_model_config: None }
    }

    #[on_event(LoadModel)]
    async fn on_load_model(
        &mut self,
        config: Input<GgmlDiffusionLoadConfig>,
        seq: BroadcastSeq,
    ) -> Result<(), GGMLDiffusionWorkerError> {
        self.handle_load_model(config.0, seq.0).await
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(
        &mut self,
        seq: BroadcastSeq,
    ) -> Result<(), GGMLDiffusionWorkerError> {
        self.handle_unload_model(seq.0).await
    }

    #[on_event(InferenceImage)]
    async fn on_inference_image(
        &mut self,
        image_params: Input<ImageGenerationRequest>,
    ) -> Result<Typed<ImageGenerationResponse>, GGMLDiffusionWorkerError> {
        self.handle_inference_image(image_params.0).await
    }

    // ── model.load ────────────────────────────────────────────────────────────

    async fn handle_load_model(
        &mut self,
        config: GgmlDiffusionLoadConfig,
        seq_id: u64,
    ) -> Result<(), GGMLDiffusionWorkerError> {
        let worker_id = self.peer_sender_id();
        let engine = match self.engine.as_mut() {
            Some(e) => e,
            None => {
                return Err(GGMLDiffusionWorkerError::load("engine not initialized"));
            }
        };
        let config_payload = Payload::typed(config.clone());

        tracing::info!(
            worker_id,
            seq_id,
            model_path = %config.model_path.display(),
            diffusion_model_path = ?config.diffusion_model_path.as_ref().map(|p| p.display().to_string()),
            vae_path = ?config.vae_path.as_ref().map(|p| p.display().to_string()),
            flash_attn = ?config.flash_attn,
            offload_params_to_cpu = ?config.offload_params_to_cpu,
            n_threads = ?config.n_threads,
            "diffusion model.load started"
        );

        let started_at = Instant::now();

        // Diffusion workers run on dedicated OS threads, so call the engine directly
        // and keep the native context pinned to that thread.
        let result = engine.new_context_from_config(config.clone());

        match result {
            Ok(()) => {
                self.last_model_config = Some(config_payload.clone());
                // Broadcast so peer workers also load the same model.
                self.emit_peer_load_model_deployment_payload(seq_id, config_payload);
                tracing::info!(
                    worker_id,
                    seq_id,
                    elapsed_ms = started_at.elapsed().as_millis(),
                    "diffusion model.load completed"
                );
                Ok(())
            }
            Err(e) => {
                tracing::error!(
                    worker_id,
                    seq_id,
                    elapsed_ms = started_at.elapsed().as_millis(),
                    error = %e,
                    "diffusion model.load failed"
                );
                Err(GGMLDiffusionWorkerError::load(e.to_string()))
            }
        }
    }

    // ── model.unload ──────────────────────────────────────────────────────────

    async fn handle_unload_model(
        &mut self,
        seq_id: u64,
    ) -> Result<(), GGMLDiffusionWorkerError> {
        match self.engine.as_mut() {
            Some(e) => {
                e.unload();
                self.last_model_config = None;
                // Broadcast so every peer worker also drops its context.
                self.emit_peer_unload_generation(seq_id);
                Ok(())
            }
            None => Err(GGMLDiffusionWorkerError::unload("engine not initialized")),
        }
    }

    // ── inference.image ───────────────────────────────────────────────────────

    async fn handle_inference_image(
        &mut self,
        image_params: ImageGenerationRequest,
    ) -> Result<Typed<ImageGenerationResponse>, GGMLDiffusionWorkerError> {
        let engine = match self.engine.as_ref() {
            Some(e) => e,
            None => {
                return Err(GGMLDiffusionWorkerError::inference("engine not initialized"));
            }
        };

        let result = engine.generate_image_from_request(image_params);

        match result {
            Err(error) => Err(GGMLDiffusionWorkerError::inference(error.to_string())),
            Ok(images) => Ok(Typed(images)),
        }
    }

    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(
        &mut self,
        config: Input<GgmlDiffusionLoadConfig>,
    ) -> Result<(), GGMLDiffusionWorkerError> {
        let config = config.0;
        let model_path = config.model_path.display().to_string();
        if let Some(engine) = self.engine.as_mut()
            && !engine.is_model_loaded()
        {
            let result = engine.new_context_from_config(config.clone());
            if let Err(e) = result {
                tracing::warn!(
                    model_path = %model_path,
                    error = %e,
                    "diffusion worker: broadcast LoadModel failed"
                );
            }
        }
        self.last_model_config = Some(Payload::typed(config));
        Ok(())
    }

    #[on_peer_control(Unload)]
    async fn on_peer_unload(&mut self) -> Result<(), GGMLDiffusionWorkerError> {
        if let Some(e) = self.engine.as_mut() {
            e.unload();
        }
        self.last_model_config = None;
        Ok(())
    }

    #[on_runtime_control(GlobalUnload)]
    #[on_runtime_control(GlobalLoad)]
    async fn apply_runtime_control(
        &mut self,
        op_id: ControlOpId,
    ) -> Result<(), GGMLDiffusionWorkerError> {
        tracing::debug!(op_id = op_id.0, "diffusion runtime control pre-cleanup");
        if let Some(engine) = self.engine.as_mut() {
            engine.unload();
        }
        self.last_model_config = None;
        Ok(())
    }

    #[on_control_lagged]
    async fn on_control_lagged(&mut self) -> Result<(), GGMLDiffusionWorkerError> {
        if let Some(e) = self.engine.as_mut() {
            e.unload();
        }
        self.last_model_config = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use slab_runtime_core::backend::DeploymentSnapshot;

    #[test]
    fn deployment_snapshot_reads_typed_ggml_diffusion_model_config() {
        let snapshot = DeploymentSnapshot::with_model(
            11,
            Payload::typed(GgmlDiffusionLoadConfig {
                model_path: PathBuf::from("model.gguf"),
                diffusion_model_path: Some(PathBuf::from("diffusion.gguf")),
                vae_path: Some(PathBuf::from("vae.gguf")),
                taesd_path: None,
                clip_l_path: None,
                clip_g_path: None,
                t5xxl_path: None,
                clip_vision_path: None,
                control_net_path: None,
                flash_attn: Some(true),
                vae_device: Some("cpu".to_owned()),
                clip_device: None,
                offload_params_to_cpu: None,
                enable_mmap: Some(true),
                n_threads: Some(8),
            }),
        );

        let config = snapshot
            .typed_model_config::<GgmlDiffusionLoadConfig>()
            .expect("typed deployment snapshot should decode");

        assert_eq!(config.model_path, PathBuf::from("model.gguf"));
        assert_eq!(config.diffusion_model_path, Some(PathBuf::from("diffusion.gguf")));
        assert_eq!(config.vae_path, Some(PathBuf::from("vae.gguf")));
        assert_eq!(config.flash_attn, Some(true));
        assert_eq!(config.vae_device.as_deref(), Some("cpu"));
        assert_eq!(config.n_threads, Some(8));
    }
}
