//! Backend worker adapter for `ggml.diffusion`.
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
//! Uses a typed [`slab_diffusion::ContextParams`] payload inside `slab-core`.

use std::time::Instant;

use slab_diffusion::{
    ContextParams as DiffusionContextParams, Image as DiffusionImage,
    ImgParams as DiffusionImgParams,
};
use tokio::sync::broadcast;

use crate::infra::backends::ggml::diffusion::adapter::GGMLDiffusionEngine;
use slab_runtime_core::Payload;
use slab_runtime_core::backend::{
    BroadcastSeq, DeploymentSnapshot, Input, PeerWorkerCommand, RuntimeControlSignal,
    SyncMessage, Typed, WorkerCommand,
};
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
    /// Broadcast sender shared among all workers so that any worker can
    /// propagate state-change commands (e.g. `Unload`) to all peers.
    bc_tx: broadcast::Sender<WorkerCommand>,
    /// Stable index used to populate `sender_id` when broadcasting.
    worker_id: usize,
    last_model_config: Option<Payload>,
}

#[backend_handler]
impl DiffusionWorker {
    pub fn new(
        engine: Option<GGMLDiffusionEngine>,
        bc_tx: broadcast::Sender<WorkerCommand>,
        worker_id: usize,
    ) -> Self {
        Self { engine, bc_tx, worker_id, last_model_config: None }
    }

    #[on_event(LoadModel)]
    async fn on_load_model(
        &mut self,
        config: Input<DiffusionContextParams>,
        seq: BroadcastSeq,
    ) -> Result<(), String> {
        self.handle_load_model(config.0, seq.0).await
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(&mut self, seq: BroadcastSeq) -> Result<(), String> {
        self.handle_unload_model(seq.0).await
    }

    #[on_event(InferenceImage)]
    async fn on_inference_image(
        &mut self,
        image_params: Input<DiffusionImgParams>,
    ) -> Result<Typed<Vec<DiffusionImage>>, String> {
        self.handle_inference_image(image_params.0).await
    }

    // ── model.load ────────────────────────────────────────────────────────────

    async fn handle_load_model(
        &mut self,
        config: DiffusionContextParams,
        seq_id: u64,
    ) -> Result<(), String> {
        let engine = match self.engine.as_mut() {
            Some(e) => e,
            None => {
                return Err("engine not initialized".to_owned());
            }
        };
        let config_payload = Payload::typed(config.clone());

        tracing::info!(
            worker_id = self.worker_id,
            seq_id,
            model_path = ?config.model_path.as_ref().map(|path| path.display().to_string()),
            diffusion_model_path = ?config.diffusion_model_path.as_ref().map(|p| p.display().to_string()),
            vae_path = ?config.vae_path.as_ref().map(|p| p.display().to_string()),
            vae_decode_only = config.vae_decode_only.unwrap_or(false),
            free_params_immediately = config.free_params_immediately.unwrap_or(false),
            tae_preview_only = config.tae_preview_only.unwrap_or(false),
            flash_attn = ?config.flash_attn,
            offload_params_to_cpu = ?config.offload_params_to_cpu,
            n_threads = ?config.n_threads,
            "diffusion model.load started"
        );

        let started_at = Instant::now();

        // Diffusion workers run on dedicated OS threads, so call the engine directly
        // and keep the native context pinned to that thread.
        let result = engine.new_context(config.clone());

        match result {
            Ok(()) => {
                self.last_model_config = Some(config_payload.clone());
                let deployment = DeploymentSnapshot::with_model(seq_id, config_payload);
                // Broadcast so peer workers also load the same model.
                let _ = self.bc_tx.send(WorkerCommand::Peer(PeerWorkerCommand::LoadModel {
                    sync: SyncMessage::Deployment(deployment),
                    sender_id: self.worker_id,
                }));
                tracing::info!(
                    worker_id = self.worker_id,
                    seq_id,
                    elapsed_ms = started_at.elapsed().as_millis(),
                    "diffusion model.load completed"
                );
                Ok(())
            }
            Err(e) => {
                tracing::error!(
                    worker_id = self.worker_id,
                    seq_id,
                    elapsed_ms = started_at.elapsed().as_millis(),
                    error = %e,
                    "diffusion model.load failed"
                );
                Err(e.to_string())
            }
        }
    }

    // ── model.unload ──────────────────────────────────────────────────────────

    async fn handle_unload_model(&mut self, seq_id: u64) -> Result<(), String> {
        match self.engine.as_mut() {
            Some(e) => {
                e.unload();
                self.last_model_config = None;
                // Broadcast so every peer worker also drops its context.
                // Ignore errors: no receivers simply means no other workers.
                let _ = self.bc_tx.send(WorkerCommand::Peer(PeerWorkerCommand::Unload {
                    sync: SyncMessage::Generation { generation: seq_id },
                    sender_id: self.worker_id,
                }));
                Ok(())
            }
            None => Err("engine not initialized".to_owned()),
        }
    }

    // ── inference.image ───────────────────────────────────────────────────────

    async fn handle_inference_image(
        &mut self,
        image_params: DiffusionImgParams,
    ) -> Result<Typed<Vec<DiffusionImage>>, String> {
        let engine = match self.engine.as_ref() {
            Some(e) => e,
            None => {
                return Err("engine not initialized".to_owned());
            }
        };

        let result = engine.generate_image(image_params);

        match result {
            Err(e) => Err(e.to_string()),
            Ok(images) => Ok(Typed(images)),
        }
    }

    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(&mut self, cmd: PeerWorkerCommand) {
        let Some(snapshot) = cmd.deployment() else {
            return;
        };
        let config: DiffusionContextParams = match snapshot.typed_model_config() {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(error = %error, "diffusion worker: invalid model deployment snapshot");
                return;
            }
        };
        let model_path = config.model_path.clone();
        if let Some(engine) = self.engine.as_mut()
            && !engine.is_model_loaded()
        {
            let result = engine.new_context(config.clone());
            if let Err(e) = result {
                tracing::warn!(
                    model_path = ?model_path.as_ref().map(|path| path.display().to_string()),
                    error = %e,
                    "diffusion worker: broadcast LoadModel failed"
                );
            }
        }
        self.last_model_config = snapshot.model.clone();
    }

    #[on_peer_control(Unload)]
    async fn on_peer_unload(&mut self) {
        if let Some(e) = self.engine.as_mut() {
            e.unload();
        }
        self.last_model_config = None;
    }

    #[on_runtime_control(GlobalUnload)]
    #[on_runtime_control(GlobalLoad)]
    async fn apply_runtime_control(&mut self, signal: RuntimeControlSignal) {
        match signal {
            RuntimeControlSignal::GlobalUnload { op_id } => {
                tracing::debug!(op_id, "diffusion runtime global unload");
                if let Some(engine) = self.engine.as_mut() {
                    engine.unload();
                }
                self.last_model_config = None;
            }
            RuntimeControlSignal::GlobalLoad { op_id, payload } => {
                let _ = payload;
                tracing::debug!(op_id, "diffusion runtime global load pre-cleanup");
                if let Some(engine) = self.engine.as_mut() {
                    engine.unload();
                }
                self.last_model_config = None;
            }
        }
    }

    #[on_control_lagged]
    async fn on_control_lagged(&mut self) {
        if let Some(e) = self.engine.as_mut() {
            e.unload();
        }
        self.last_model_config = None;
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn deployment_snapshot_reads_typed_ggml_diffusion_model_config() {
        let snapshot = DeploymentSnapshot::with_model(
            11,
            Payload::typed(DiffusionContextParams {
                model_path: Some(PathBuf::from("model.gguf")),
                diffusion_model_path: Some(PathBuf::from("diffusion.gguf")),
                vae_path: Some(PathBuf::from("vae.gguf")),
                flash_attn: Some(true),
                vae_device: Some("cpu".to_owned()),
                enable_mmap: Some(true),
                n_threads: Some(8),
                ..Default::default()
            }),
        );

        let config = snapshot
            .typed_model_config::<DiffusionContextParams>()
            .expect("typed deployment snapshot should decode");

        assert_eq!(config.model_path, Some(PathBuf::from("model.gguf")));
        assert_eq!(config.diffusion_model_path, Some(PathBuf::from("diffusion.gguf")));
        assert_eq!(config.vae_path, Some(PathBuf::from("vae.gguf")));
        assert_eq!(config.flash_attn, Some(true));
        assert_eq!(config.vae_device.as_deref(), Some("cpu"));
        assert_eq!(config.n_threads, Some(8));
    }
}
