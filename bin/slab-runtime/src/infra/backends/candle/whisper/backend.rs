//! Backend worker for `candle.whisper`.
//!
//! Mirrors the `ggml.whisper` backend contract.
//!
//! # Supported ops
//!
//! | Op string        | Event variant | Description                                    |
//! |------------------|---------------|------------------------------------------------|
//! | `"model.load"`   | `LoadModel`   | Load Whisper model weights from disk.          |
//! | `"model.unload"` | `UnloadModel` | Drop model weights from memory.                |
//! | `"inference"`    | `Inference`   | Transcribe f32 PCM audio; returns raw text.    |
//!
//! ### `model.load` input payload
//! Uses a typed runtime-owned `CandleWhisperLoadConfig` payload inside `slab-runtime`.

use std::sync::Arc;

use tokio::sync::broadcast;

use crate::domain::models::CandleWhisperLoadConfig;
use crate::infra::backends::candle::whisper::adapter::CandleWhisperEngine;
use slab_runtime_core::Payload;
use slab_runtime_core::backend::spawn_workers;
use slab_runtime_core::backend::{
    BackendReply, BackendRequest, DeploymentSnapshot, PeerWorkerCommand, RuntimeControlSignal,
    SyncMessage, WorkerCommand,
};
use slab_runtime_macros::backend_handler;

// ── Worker ────────────────────────────────────────────────────────────────────

pub(crate) struct CandleWhisperWorker {
    engine: Option<CandleWhisperEngine>,
    bc_tx: broadcast::Sender<WorkerCommand>,
    worker_id: usize,
}

#[backend_handler]
impl CandleWhisperWorker {
    pub(crate) fn new(
        engine: Option<CandleWhisperEngine>,
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

    #[on_event(Inference)]
    async fn on_inference(&mut self, req: BackendRequest) {
        let BackendRequest { input, reply_tx, .. } = req;
        self.handle_inference(input, reply_tx).await;
    }

    // ── Runtime / peer control ────────────────────────────────────────────────

    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(&mut self, cmd: PeerWorkerCommand) {
        let Some(snapshot) = cmd.deployment() else {
            return;
        };
        let config: CandleWhisperLoadConfig = match snapshot.typed_model_config() {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(error = %error, "candle.whisper worker: invalid deployment snapshot");
                return;
            }
        };
        let model_path = config.model_path;
        let tokenizer_path = config.tokenizer_path;
        let PeerWorkerCommand::LoadModel { .. } = cmd else {
            return;
        };
        if let Some(engine) = self.engine.as_ref()
            && !engine.is_model_loaded()
        {
            let engine = engine.clone();
            let result = tokio::task::block_in_place(|| {
                engine.load_model(&model_path, tokenizer_path.as_deref())
            });
            if let Err(e) = result {
                tracing::warn!(
                    model_path = %model_path.display(),
                    error = %e,
                    "candle.whisper worker: broadcast LoadModel failed"
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
                tracing::debug!(op_id, "candle.whisper runtime global unload");
                if let Some(e) = self.engine.as_ref() {
                    e.unload();
                }
            }
            RuntimeControlSignal::GlobalLoad { op_id, payload } => {
                let _ = payload;
                tracing::debug!(op_id, "candle.whisper runtime global load pre-cleanup");
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
        let config: CandleWhisperLoadConfig = match input.to_typed() {
            Ok(c) => c,
            Err(e) => {
                let _ =
                    reply_tx.send(BackendReply::Error(format!("invalid model.load config: {e}")));
                return;
            }
        };

        let engine = self.engine.get_or_insert_with(CandleWhisperEngine::new);
        let tokenizer_path = config.tokenizer_path;
        let model_path = config.model_path;
        let engine_clone = engine.clone();

        let result = tokio::task::block_in_place(move || {
            engine_clone.load_model(&model_path, tokenizer_path.as_deref())
        });

        match result {
            Ok(()) => {
                let _ = self.bc_tx.send(WorkerCommand::Peer(PeerWorkerCommand::LoadModel {
                    sync: SyncMessage::Deployment(deployment),
                    sender_id: self.worker_id,
                }));
                let _ = reply_tx.send(BackendReply::Ack);
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
                let _ = reply_tx.send(BackendReply::Ack);
            }
            None => {
                let _ = reply_tx.send(BackendReply::Error("model not loaded".into()));
            }
        }
    }

    async fn handle_inference(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
    ) {
        let engine = match self.engine.as_ref() {
            Some(e) => e.clone(),
            None => {
                let _ = reply_tx.send(BackendReply::Error(
                    "candle.whisper backend not ready: model not loaded. Call model.load first"
                        .into(),
                ));
                return;
            }
        };

        let samples = match input.to_f32_arc() {
            Ok(s) => s,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "invalid input: expected f32 PCM audio samples, got: {e}"
                )));
                return;
            }
        };

        if samples.is_empty() {
            let _ =
                reply_tx.send(BackendReply::Error("invalid input: audio samples are empty".into()));
            return;
        }

        let result = tokio::task::block_in_place(|| engine.inference(&samples));

        match result {
            Ok(text) => {
                let _ =
                    reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from(text.as_bytes()))));
            }
            Err(e) => {
                let _ = reply_tx
                    .send(BackendReply::Error(format!("candle.whisper inference failed: {e}")));
            }
        }
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Spawn `count` Candle Whisper backend workers.
pub fn spawn_backend(
    shared_ingress_rx: slab_runtime_core::backend::SharedIngressRx,
    control_tx: broadcast::Sender<WorkerCommand>,
    count: usize,
) {
    spawn_workers(shared_ingress_rx, control_tx, count.max(1), |worker_id, bc_tx| {
        CandleWhisperWorker::new(Some(CandleWhisperEngine::new()), bc_tx, worker_id)
    });
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::CandleWhisperWorker;
    use crate::domain::models::CandleWhisperLoadConfig;
    use slab_runtime_core::Payload;
    use slab_runtime_core::backend::DeploymentSnapshot;
    use slab_runtime_core::backend::RuntimeControlSignal;
    use slab_runtime_core::backend::WorkerCommand;
    use tokio::sync::broadcast;

    fn make_worker() -> CandleWhisperWorker {
        let (bc_tx, _bc_rx) = broadcast::channel::<WorkerCommand>(8);
        CandleWhisperWorker::new(None, bc_tx, 0)
    }

    #[test]
    fn deployment_snapshot_reads_typed_candle_whisper_model_config() {
        let snapshot = DeploymentSnapshot::with_model(
            5,
            Payload::typed(CandleWhisperLoadConfig {
                model_path: PathBuf::from("model.safetensors"),
                tokenizer_path: Some(PathBuf::from("tokenizer.json")),
            }),
        );

        let config = snapshot
            .typed_model_config::<CandleWhisperLoadConfig>()
            .expect("typed deployment snapshot should decode");

        assert_eq!(config.model_path, PathBuf::from("model.safetensors"));
        assert_eq!(config.tokenizer_path, Some(PathBuf::from("tokenizer.json")));
    }

    #[tokio::test]
    async fn global_unload_is_safe_without_engine() {
        let mut worker = make_worker();
        worker.apply_runtime_control(RuntimeControlSignal::GlobalUnload { op_id: 1 }).await;
        // No panic – test passes.
    }
}
