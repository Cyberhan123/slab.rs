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
//! | `"inference.image"` | `InferenceImage` | Generate an image; input is JSON params.      |
//!
//! ### `model.load` input JSON
//! ```json
//! { "model_path": "/path/to/unet.safetensors", "vae_path": null }
//! ```
//!
//! ### `inference.image` input JSON
//! ```json
//! {
//!   "prompt": "a lovely cat",
//!   "negative_prompt": "",
//!   "width": 512,
//!   "height": 512,
//!   "cfg_scale": 7.5,
//!   "sample_steps": 20,
//!   "seed": 42
//! }
//! ```

use std::sync::Arc;

use base64::Engine as _;
use serde::Deserialize;
use tokio::sync::broadcast;

use crate::internal::engine::candle::config::CandleDiffusionModelLoadConfig;
use crate::internal::engine::candle::diffusion::adapter::{CandleDiffusionEngine, GenImageParams};
use crate::internal::scheduler::backend::protocol::{
    BackendReply, BackendRequest, DeploymentSnapshot, PeerWorkerCommand, RuntimeControlSignal,
    SyncMessage, WorkerCommand,
};
use crate::internal::scheduler::backend::runner::spawn_workers;
use crate::internal::scheduler::types::Payload;
use slab_core_macros::backend_handler;

// ── Input parameters ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct GenImageInput {
    prompt: String,
    #[serde(default)]
    negative_prompt: String,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default = "default_steps")]
    sample_steps: usize,
    #[serde(default = "default_cfg_scale")]
    cfg_scale: f64,
    #[serde(default = "default_seed")]
    seed: u64,
}

fn default_width() -> u32 {
    512
}
fn default_height() -> u32 {
    512
}
fn default_steps() -> usize {
    20
}
fn default_cfg_scale() -> f64 {
    7.5
}
fn default_seed() -> u64 {
    42
}

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
        let config: CandleDiffusionModelLoadConfig = match snapshot.model_config() {
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
        if let Some(engine) = self.engine.as_ref() {
            if !engine.is_model_loaded() {
                let engine_clone = engine.clone();
                let result = tokio::task::block_in_place(|| {
                    engine_clone.load_model(&model_path, vae_path.as_deref(), &sd_version)
                });
                if let Err(e) = result {
                    tracing::warn!(
                        model_path,
                        error = %e,
                        "candle.diffusion worker: broadcast LoadModel failed"
                    );
                }
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
        let config: CandleDiffusionModelLoadConfig = match input.to_json() {
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

        let raw: GenImageInput = match input.to_json() {
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
            negative_prompt: raw.negative_prompt,
            width: raw.width,
            height: raw.height,
            steps: raw.sample_steps,
            cfg_scale: raw.cfg_scale,
            seed: raw.seed,
        };

        let result = tokio::task::block_in_place(move || engine.inference(&params));

        match result {
            Ok(png_bytes) => {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
                let json = serde_json::json!({
                    "images": [{
                        "image": b64,
                    }]
                });
                let _ = reply_tx.send(BackendReply::Value(Payload::Json(json)));
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
    use super::CandleDiffusionWorker;
    use crate::internal::scheduler::backend::protocol::{RuntimeControlSignal, WorkerCommand};
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
}
