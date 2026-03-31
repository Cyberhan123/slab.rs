//! Backend worker for `candle.diffusion`.
//!
//! Mirrors the `ggml.diffusion` backend contract.
//!
//! # Supported ops
//!
//! | Op string           | Event variant    | Description                                   |
//! |---------------------|------------------|-----------------------------------------------|
//! | `"model.load"`      | `LoadModel`      | Load Stable Diffusion model weights.          |
//! | `"model.unload"`    | `UnloadModel`    | Drop model weights from memory.               |
//! | `"inference.image"` | `InferenceImage` | Generate an image from typed diffusion params. |
//!
//! ### `model.load` input payload
//! Uses a typed [`slab_types::CandleDiffusionLoadConfig`] payload inside `slab-core`.
//!
use std::sync::Arc;

use slab_types::{
    CandleDiffusionLoadConfig, DiffusionImageRequest, DiffusionImageResponse, GeneratedImage,
};
use tokio::sync::broadcast;

use crate::internal::engine::candle::diffusion::adapter::{CandleDiffusionEngine, GenImageParams};
use crate::internal::scheduler::backend::protocol::{
    BackendReply, BackendRequest, DeploymentSnapshot, PeerWorkerCommand, RuntimeControlSignal,
    SyncMessage, WorkerCommand,
};
use crate::internal::scheduler::backend::runner::spawn_workers;
use crate::internal::scheduler::types::Payload;
use slab_core_macros::backend_handler;

// ── Input parameters ──────────────────────────────────────────────────────────

// ── Worker ────────────────────────────────────────────────────────────────────

pub(crate) struct CandleDiffusionWorker {
    engine: Option<CandleDiffusionEngine>,
    bc_tx: broadcast::Sender<WorkerCommand>,
    worker_id: usize,
}

#[backend_handler]
impl CandleDiffusionWorker {
    pub(crate) fn new(
        engine: Option<CandleDiffusionEngine>,
        bc_tx: broadcast::Sender<WorkerCommand>,
        worker_id: usize,
    ) -> Self {
        Self { engine, bc_tx, worker_id }
    }

    #[on_event(LoadModel)]
    async fn on_load_model(&mut self, req: BackendRequest) {
        let BackendRequest { input, broadcast_seq, reply_tx, .. } = req;
        let seq_id = broadcast_seq.unwrap_or(0);
        self.handle_load_model(input, reply_tx, seq_id).await;
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(&mut self, req: BackendRequest) {
        let BackendRequest { broadcast_seq, reply_tx, .. } = req;
        let seq_id = broadcast_seq.unwrap_or(0);
        self.handle_unload_model(reply_tx, seq_id).await;
    }

    #[on_event(InferenceImage)]
    async fn on_inference_image(&mut self, req: BackendRequest) {
        let BackendRequest { input, reply_tx, .. } = req;
        self.handle_inference_image(input, reply_tx).await;
    }

    // ── Runtime / peer control ────────────────────────────────────────────────

    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(&mut self, cmd: PeerWorkerCommand) {
        let Some(snapshot) = cmd.deployment() else {
            return;
        };
        let config: CandleDiffusionLoadConfig = match snapshot.typed_model_config() {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(error = %error, "candle.diffusion worker: invalid deployment snapshot");
                return;
            }
        };
        let model_path = config.model_path;
        let vae_path = config.vae_path;
        let sd_version = config.sd_version;
        let PeerWorkerCommand::LoadModel { .. } = cmd else {
            return;
        };
        if let Some(engine) = self.engine.as_ref()
            && !engine.is_model_loaded()
        {
            let engine_clone = engine.clone();
            let result = tokio::task::block_in_place(|| {
                engine_clone.load_model(&model_path, vae_path.as_deref(), &sd_version)
            });
            if let Err(e) = result {
                tracing::warn!(
                    model_path = %model_path.display(),
                    error = %e,
                    "candle.diffusion worker: broadcast LoadModel failed"
                );
            }
        }
    }

    #[on_peer_control(Unload)]
    async fn on_peer_unload(&mut self) {
        if let Some(e) = self.engine.as_ref() {
            e.unload();
        }
    }

    #[on_runtime_control(GlobalUnload)]
    #[on_runtime_control(GlobalLoad)]
    async fn apply_runtime_control(&mut self, signal: RuntimeControlSignal) {
        match signal {
            RuntimeControlSignal::GlobalUnload { op_id } => {
                tracing::debug!(op_id, "candle.diffusion runtime global unload");
                if let Some(e) = self.engine.as_ref() {
                    e.unload();
                }
            }
            RuntimeControlSignal::GlobalLoad { op_id, payload } => {
                let _ = payload;
                tracing::debug!(op_id, "candle.diffusion runtime global load pre-cleanup");
                if let Some(e) = self.engine.as_ref() {
                    e.unload();
                }
            }
        }
    }

    #[on_control_lagged]
    async fn on_control_lagged(&mut self) {
        if let Some(e) = self.engine.as_ref() {
            e.unload();
        }
    }

    // ── Handler helpers ───────────────────────────────────────────────────────

    async fn handle_load_model(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
        seq_id: u64,
    ) {
        let deployment = DeploymentSnapshot::with_model(seq_id, input.clone());
        let config: CandleDiffusionLoadConfig = match input.to_typed() {
            Ok(c) => c,
            Err(e) => {
                let _ =
                    reply_tx.send(BackendReply::Error(format!("invalid model.load config: {e}")));
                return;
            }
        };

        let engine = self.engine.get_or_insert_with(CandleDiffusionEngine::new).clone();
        let model_path = config.model_path.clone();
        let vae_path = config.vae_path.clone();
        let sd_version = config.sd_version.clone();

        let result = tokio::task::block_in_place(move || {
            engine.load_model(&model_path, vae_path.as_deref(), &sd_version)
        });

        match result {
            Ok(()) => {
                let _ = self.bc_tx.send(WorkerCommand::Peer(PeerWorkerCommand::LoadModel {
                    sync: SyncMessage::Deployment(deployment),
                    sender_id: self.worker_id,
                }));
                let _ =
                    reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from([] as [u8; 0]))));
            }
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
            }
        }
    }

    async fn handle_unload_model(
        &mut self,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
        seq_id: u64,
    ) {
        match self.engine.as_ref() {
            Some(engine) => {
                engine.unload();
                let _ = self.bc_tx.send(WorkerCommand::Peer(PeerWorkerCommand::Unload {
                    sync: SyncMessage::Generation { generation: seq_id },
                    sender_id: self.worker_id,
                }));
                let _ =
                    reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from([] as [u8; 0]))));
            }
            None => {
                let _ = reply_tx.send(BackendReply::Error("model not loaded".into()));
            }
        }
    }

    async fn handle_inference_image(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
    ) {
        let engine = match self.engine.as_ref() {
            Some(e) => e.clone(),
            None => {
                let _ = reply_tx.send(BackendReply::Error(
                    "candle.diffusion backend not ready: model not loaded".into(),
                ));
                return;
            }
        };

        let raw: DiffusionImageRequest = match input.to_typed() {
            Ok(v) => v,
            Err(e) => {
                let _ = reply_tx
                    .send(BackendReply::Error(format!("invalid inference.image params: {e}")));
                return;
            }
        };

        if raw.prompt.trim().is_empty() {
            let _ = reply_tx.send(BackendReply::Error("prompt must not be empty".into()));
            return;
        }

        let params = GenImageParams {
            prompt: raw.prompt,
            negative_prompt: raw.negative_prompt.unwrap_or_default(),
            width: raw.width,
            height: raw.height,
            steps: raw.steps.unwrap_or(20).max(1) as usize,
            cfg_scale: f64::from(raw.cfg_scale.or(raw.guidance).unwrap_or(7.5)),
            seed: raw.seed.and_then(|value| u64::try_from(value).ok()).unwrap_or(42),
        };
        let output_width = params.width;
        let output_height = params.height;

        let result = tokio::task::block_in_place(move || engine.inference(&params));

        match result {
            Ok(png_bytes) => {
                let response = DiffusionImageResponse {
                    images: vec![GeneratedImage {
                        bytes: png_bytes,
                        width: output_width,
                        height: output_height,
                        channels: 3,
                    }],
                    metadata: Default::default(),
                };
                let _ = reply_tx.send(BackendReply::Value(Payload::typed(response)));
            }
            Err(e) => {
                let _ = reply_tx
                    .send(BackendReply::Error(format!("candle.diffusion inference failed: {e}")));
            }
        }
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Spawn `count` Candle Diffusion backend workers.
pub(crate) fn spawn_backend(
    shared_ingress_rx: crate::internal::scheduler::backend::runner::SharedIngressRx,
    control_tx: broadcast::Sender<WorkerCommand>,
    count: usize,
) {
    spawn_workers(shared_ingress_rx, control_tx, count.max(1), |worker_id, bc_tx| {
        CandleDiffusionWorker::new(Some(CandleDiffusionEngine::new()), bc_tx, worker_id)
    });
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::CandleDiffusionWorker;
    use crate::internal::scheduler::backend::protocol::{
        DeploymentSnapshot, RuntimeControlSignal, WorkerCommand,
    };
    use crate::internal::scheduler::types::Payload;
    use slab_types::CandleDiffusionLoadConfig;
    use tokio::sync::broadcast;

    fn make_worker() -> CandleDiffusionWorker {
        let (bc_tx, _) = broadcast::channel::<WorkerCommand>(8);
        CandleDiffusionWorker::new(None, bc_tx, 0)
    }

    #[tokio::test]
    async fn global_unload_is_safe_without_engine() {
        let mut worker = make_worker();
        worker.apply_runtime_control(RuntimeControlSignal::GlobalUnload { op_id: 1 }).await;
        // No panic – test passes.
    }

    #[test]
    fn deployment_snapshot_reads_typed_candle_diffusion_model_config() {
        let snapshot = DeploymentSnapshot::with_model(
            6,
            Payload::typed(CandleDiffusionLoadConfig {
                model_path: PathBuf::from("model.safetensors"),
                vae_path: Some(PathBuf::from("vae.safetensors")),
                sd_version: "v1-5".to_owned(),
            }),
        );

        let config = snapshot
            .typed_model_config::<CandleDiffusionLoadConfig>()
            .expect("typed deployment snapshot should decode");

        assert_eq!(config.model_path, PathBuf::from("model.safetensors"));
        assert_eq!(config.vae_path, Some(PathBuf::from("vae.safetensors")));
        assert_eq!(config.sd_version, "v1-5");
    }
}
