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
//! ### `model.load` input payload
//! Uses a typed runtime-owned `OnnxLoadConfig` payload inside `slab-runtime`.
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

use tokio::sync::broadcast;
use tracing::warn;

use crate::domain::models::OnnxLoadConfig;
use crate::infra::backends::onnx::adapter::OnnxEngine;
use crate::infra::backends::onnx::config::OnnxInferenceInput;
use slab_runtime_core::Payload;
use slab_runtime_core::backend::{
    BroadcastSeq, ControlOpId, DeploymentSnapshot, Input, PeerWorkerCommand, SyncMessage,
    WorkerCommand,
};
use slab_runtime_macros::backend_handler;

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
pub struct OnnxWorker {
    /// The ONNX engine.  `is_loaded() == false` when no model is loaded.
    engine: OnnxEngine,
    /// The config used to load the current model.  Stored so that peer workers
    /// receiving a broadcast can reproduce the same session configuration
    /// (execution providers, thread counts, etc.).
    current_config: Option<OnnxLoadConfig>,
    /// Broadcast sender shared among all workers.
    bc_tx: broadcast::Sender<WorkerCommand>,
    /// Stable index used to populate `sender_id` when broadcasting.
    worker_id: usize,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn deployment_snapshot_reads_typed_onnx_model_config() {
        let snapshot = DeploymentSnapshot::with_model(
            9,
            Payload::typed(OnnxLoadConfig {
                model_path: PathBuf::from("model.onnx"),
                execution_providers: vec!["CPU".to_owned()],
                intra_op_num_threads: Some(4),
                inter_op_num_threads: None,
            }),
        );

        let config = snapshot
            .typed_model_config::<OnnxLoadConfig>()
            .expect("typed deployment snapshot should decode");

        assert_eq!(config.model_path, PathBuf::from("model.onnx"));
        assert_eq!(config.execution_providers, vec!["CPU".to_owned()]);
        assert_eq!(config.intra_op_num_threads, Some(4));
        assert_eq!(config.inter_op_num_threads, None);
    }
}

#[backend_handler]
impl OnnxWorker {
    pub fn new(bc_tx: broadcast::Sender<WorkerCommand>, worker_id: usize) -> Self {
        Self { engine: OnnxEngine::new(), current_config: None, bc_tx, worker_id }
    }

    // ── dispatch ──────────────────────────────────────────────────────────────

    #[on_event(LoadModel)]
    async fn on_load_model(
        &mut self,
        config: Input<OnnxLoadConfig>,
        seq: BroadcastSeq,
    ) -> Result<(), String> {
        self.handle_load_model(config.0, seq.0).await
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(&mut self, seq: BroadcastSeq) -> Result<(), String> {
        self.handle_unload_model(seq.0).await
    }

    #[on_event(Inference)]
    async fn on_inference(
        &mut self,
        input: Input<OnnxInferenceInput>,
    ) -> Result<serde_json::Value, String> {
        self.handle_inference(input.0).await
    }

    // ── model.load ────────────────────────────────────────────────────────────

    async fn handle_load_model(
        &mut self,
        config: OnnxLoadConfig,
        seq_id: u64,
    ) -> Result<(), String> {
        let deployment = DeploymentSnapshot::with_model(seq_id, Payload::typed(config.clone()));
        // Session creation is CPU / I/O-bound; run on the blocking thread pool.
        let result = tokio::task::block_in_place(|| self.engine.load_model(config.clone()));

        match result {
            Ok(()) => {
                // Store the full config so peer workers can replicate it.
                self.current_config = Some(config);
                // Broadcast so peer workers also load the same model.
                let _ = self.bc_tx.send(WorkerCommand::Peer(PeerWorkerCommand::LoadModel {
                    sync: SyncMessage::Deployment(deployment),
                    sender_id: self.worker_id,
                }));
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    // ── model.unload ──────────────────────────────────────────────────────────

    async fn handle_unload_model(&mut self, seq_id: u64) -> Result<(), String> {
        self.engine.unload();
        self.current_config = None;
        // Broadcast so peer workers also drop their sessions.
        let _ = self.bc_tx.send(WorkerCommand::Peer(PeerWorkerCommand::Unload {
            sync: SyncMessage::Generation { generation: seq_id },
            sender_id: self.worker_id,
        }));
        Ok(())
    }

    // ── inference ─────────────────────────────────────────────────────────────

    async fn handle_inference(
        &mut self,
        input: OnnxInferenceInput,
    ) -> Result<serde_json::Value, String> {
        // Inference is CPU-bound; run on the blocking thread pool.
        let result = tokio::task::block_in_place(|| self.engine.inference(input));

        match result {
            Ok(json_output) => Ok(json_output),
            Err(e) => {
                warn!(error = %e, "ONNX inference error");
                Err(e.to_string())
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
    async fn on_peer_load_model(&mut self, config: Input<OnnxLoadConfig>) -> Result<(), String> {
        let config = config.0;
        let model_path = config.model_path.clone();

        // Short-circuit: same model already loaded.
        if let Some(cfg) = &self.current_config
            && cfg.model_path == model_path
        {
            return Ok(());
        }

        self.engine.unload();
        let result = tokio::task::block_in_place(|| self.engine.load_model(config.clone()));
        match result {
            Ok(()) => {
                self.current_config = Some(config);
            }
            Err(e) => {
                self.current_config = None;
                warn!(
                    model_path = %model_path.display(),
                    error = %e,
                    "ONNX worker: broadcast LoadModel failed"
                );
            }
        }
        Ok(())
    }

    /// When another worker unloads the model, drop the session in this worker.
    #[on_peer_control(Unload)]
    async fn on_peer_unload(&mut self) -> Result<(), String> {
        self.engine.unload();
        self.current_config = None;
        Ok(())
    }

    // ── global runtime control ────────────────────────────────────────────────

    #[on_runtime_control(GlobalUnload)]
    #[on_runtime_control(GlobalLoad)]
    async fn apply_runtime_control(&mut self, op_id: ControlOpId) -> Result<(), String> {
        tracing::debug!(op_id = op_id.0, "ONNX runtime control pre-cleanup");
        self.engine.unload();
        self.current_config = None;
        Ok(())
    }

    /// Conservative unload when broadcast channel lags – avoid running stale
    /// inference on a model that peers may have already replaced.
    #[on_control_lagged]
    async fn on_control_lagged(&mut self) -> Result<(), String> {
        self.engine.unload();
        self.current_config = None;
        Ok(())
    }
}
