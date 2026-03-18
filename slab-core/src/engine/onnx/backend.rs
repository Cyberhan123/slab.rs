//! Backend worker adapter for `onnx`.
//!
//! Defines [`OnnxWorker`] – a backend handler driven by the slab-core
//! scheduler.
//!
//! # Supported ops
//!
//! | Op string        | Event variant  | Description                                        |
//! |------------------|----------------|----------------------------------------------------|
//! | `"model.load"`   | `LoadModel`    | Load an ONNX model file and create a session.      |
//! | `"model.unload"` | `UnloadModel`  | Drop the session and free model memory.            |
//! | `"inference"`    | `Inference`    | Run a forward pass; input and output are JSON.     |
//!
//! ### `model.load` input JSON
//! ```json
//! {
//!   "model_path": "/models/resnet50.onnx",
//!   "execution_providers": ["CUDA", "CPU"],
//!   "intra_op_num_threads": 4,
//!   "inter_op_num_threads": 1
//! }
//! ```
//!
//! ### `inference` input JSON
//! ```json
//! {
//!   "inputs": {
//!     "pixel_values": {
//!       "shape": [1, 3, 224, 224],
//!       "dtype": "float32",
//!       "data_b64": "<base64-encoded little-endian bytes>"
//!     }
//!   }
//! }
//! ```
//!
//! ### `inference` output JSON
//! ```json
//! {
//!   "outputs": {
//!     "logits": {
//!       "shape": [1, 1000],
//!       "dtype": "float32",
//!       "data_b64": "<base64-encoded little-endian bytes>"
//!     }
//!   }
//! }
//! ```

use std::sync::Arc;

use tokio::sync::broadcast;
use tracing::warn;

use crate::engine::onnx::adapter::OnnxEngine;
use crate::engine::onnx::config::{OnnxInferenceInput, OnnxModelLoadConfig};
use crate::scheduler::backend::backend_handler;
use crate::scheduler::backend::protocol::{
    BackendReply, BackendRequest, PeerWorkerCommand, RuntimeControlSignal, WorkerCommand,
};
use crate::scheduler::types::Payload;

// ── Worker ────────────────────────────────────────────────────────────────────

/// A single ONNX backend worker.
///
/// Unlike the GGML workers, the ONNX worker does **not** have a separate
/// library loading step – ONNX Runtime is managed internally by the `ort`
/// crate.  Each worker owns its own [`OnnxEngine`] and there is no shared
/// mutable state across workers, so no locking is needed.
///
/// When multiple workers are spawned they each load an independent session
/// from the same model file (sessions are not shared or cloned).  The
/// `broadcast` channel is still used to propagate `model.load` and
/// `model.unload` commands to peer workers so all sessions stay in sync.
pub(crate) struct OnnxWorker {
    /// The ONNX engine.  `is_loaded() == false` when no model is loaded.
    engine: OnnxEngine,
    /// The config used to load the current model.  Stored so that peer workers
    /// receiving a broadcast can reproduce the same session configuration
    /// (execution providers, thread counts, etc.).
    current_config: Option<OnnxModelLoadConfig>,
    /// Broadcast sender shared among all workers.
    bc_tx: broadcast::Sender<WorkerCommand>,
    /// Stable index used to populate `sender_id` when broadcasting.
    worker_id: usize,
}

#[backend_handler]
impl OnnxWorker {
    pub(crate) fn new(
        bc_tx: broadcast::Sender<WorkerCommand>,
        worker_id: usize,
    ) -> Self {
        Self {
            engine: OnnxEngine::new(),
            current_config: None,
            bc_tx,
            worker_id,
        }
    }

    // ── dispatch ──────────────────────────────────────────────────────────────

    #[on_event(LoadModel)]
    async fn on_load_model(&mut self, req: BackendRequest) {
        let BackendRequest {
            input,
            broadcast_seq,
            reply_tx,
            ..
        } = req;
        let seq_id = broadcast_seq.unwrap_or(0);
        self.handle_load_model(input, reply_tx, seq_id).await;
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(&mut self, req: BackendRequest) {
        let BackendRequest {
            broadcast_seq,
            reply_tx,
            ..
        } = req;
        let seq_id = broadcast_seq.unwrap_or(0);
        self.handle_unload_model(reply_tx, seq_id).await;
    }

    #[on_event(Inference)]
    async fn on_inference(&mut self, req: BackendRequest) {
        let BackendRequest {
            input, reply_tx, ..
        } = req;
        self.handle_inference(input, reply_tx).await;
    }

    // ── model.load ────────────────────────────────────────────────────────────

    async fn handle_load_model(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
        seq_id: u64,
    ) {
        let config: OnnxModelLoadConfig = match input.to_json() {
            Ok(c) => c,
            Err(e) => {
                let _ = reply_tx
                    .send(BackendReply::Error(format!("invalid model.load config: {e}")));
                return;
            }
        };

        let model_path = config.model_path.clone();

        // Session creation is CPU / I/O-bound; run on the blocking thread pool.
        let result = tokio::task::block_in_place(|| self.engine.load_model(config.clone()));

        match result {
            Ok(()) => {
                // Store the full config so peer workers can replicate it.
                self.current_config = Some(config);
                // Broadcast so peer workers also load the same model.
                let _ = self
                    .bc_tx
                    .send(WorkerCommand::Peer(PeerWorkerCommand::LoadModel {
                        model_path,
                        sender_id: self.worker_id,
                        seq_id,
                    }));
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(
                    Arc::from([] as [u8; 0]),
                )));
            }
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
            }
        }
    }

    // ── model.unload ──────────────────────────────────────────────────────────

    async fn handle_unload_model(
        &mut self,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
        seq_id: u64,
    ) {
        self.engine.unload();
        self.current_config = None;
        // Broadcast so peer workers also drop their sessions.
        let _ = self
            .bc_tx
            .send(WorkerCommand::Peer(PeerWorkerCommand::Unload {
                sender_id: self.worker_id,
                seq_id,
            }));
        let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(
            Arc::from([] as [u8; 0]),
        )));
    }

    // ── inference ─────────────────────────────────────────────────────────────

    async fn handle_inference(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
    ) {
        let inference_input: OnnxInferenceInput = match input.to_json() {
            Ok(p) => p,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "invalid inference payload: {e}"
                )));
                return;
            }
        };

        // Inference is CPU-bound; run on the blocking thread pool.
        let result = tokio::task::block_in_place(|| self.engine.inference(inference_input));

        match result {
            Ok(json_output) => {
                let payload = Payload::Json(json_output);
                let _ = reply_tx.send(BackendReply::Value(payload));
            }
            Err(e) => {
                warn!(error = %e, "ONNX inference error");
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
            }
        }
    }

    // ── peer broadcast handlers ───────────────────────────────────────────────

    /// When another worker loads a model, replicate the load in this worker
    /// (each worker needs its own independent session).
    ///
    /// If the worker already has a **different** model loaded, it is
    /// replaced.  If it already has the **same** model loaded, the call is a
    /// no-op to avoid unnecessary session teardown.
    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(&mut self, cmd: PeerWorkerCommand) {
        let PeerWorkerCommand::LoadModel { model_path, .. } = cmd else {
            return;
        };

        // Short-circuit: same model already loaded.
        if let Some(cfg) = &self.current_config {
            if cfg.model_path == model_path {
                return;
            }
        }

        // Different model (or no model): unload current and load the new one.
        // Reuse the execution provider / thread config from the originating
        // worker's load call; fall back to CPU-only defaults if no config is
        // stored yet (e.g. this is a fresh peer that never loaded a model).
        let config = self
            .current_config
            .as_ref()
            .map(|c| OnnxModelLoadConfig {
                model_path: model_path.clone(),
                execution_providers: c.execution_providers.clone(),
                intra_op_num_threads: c.intra_op_num_threads,
                inter_op_num_threads: c.inter_op_num_threads,
            })
            .unwrap_or_else(|| OnnxModelLoadConfig {
                model_path: model_path.clone(),
                execution_providers: vec!["CPU".to_string()],
                intra_op_num_threads: 0,
                inter_op_num_threads: 0,
            });

        self.engine.unload();
        let result = tokio::task::block_in_place(|| self.engine.load_model(config.clone()));
        match result {
            Ok(()) => {
                self.current_config = Some(config);
            }
            Err(e) => {
                self.current_config = None;
                warn!(
                    model_path,
                    error = %e,
                    "ONNX worker: broadcast LoadModel failed"
                );
            }
        }
    }

    /// When another worker unloads the model, drop the session in this worker.
    #[on_peer_control(Unload)]
    async fn on_peer_unload(&mut self) {
        self.engine.unload();
        self.current_config = None;
    }

    // ── global runtime control ────────────────────────────────────────────────

    #[on_runtime_control(GlobalUnload)]
    #[on_runtime_control(GlobalLoad)]
    async fn apply_runtime_control(&mut self, signal: RuntimeControlSignal) {
        match signal {
            RuntimeControlSignal::GlobalUnload { op_id } => {
                tracing::debug!(op_id, "ONNX runtime global unload");
                self.engine.unload();
                self.current_config = None;
            }
            RuntimeControlSignal::GlobalLoad { op_id, payload } => {
                let _ = payload;
                tracing::debug!(op_id, "ONNX runtime global load pre-cleanup");
                self.engine.unload();
                self.current_config = None;
            }
        }
    }

    /// Conservative unload when broadcast channel lags – avoid running stale
    /// inference on a model that peers may have already replaced.
    #[on_control_lagged]
    async fn on_control_lagged(&mut self) {
        self.engine.unload();
        self.current_config = None;
    }
}

