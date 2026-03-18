//! Backend worker for `candle.whisper`.
//!
//! Mirrors the `ggml.whisper` backend contract.  The `lib.load` / `lib.reload`
//! ops are accepted as no-ops since the Candle crate is statically linked.
//!
//! # Supported ops
//!
//! | Op string        | Event variant | Description                                    |
//! |------------------|---------------|------------------------------------------------|
//! | `"lib.load"`     | `LoadLibrary` | No-op (Candle is statically linked).           |
//! | `"lib.reload"`   | `ReloadLibrary` | No-op.                                       |
//! | `"model.load"`   | `LoadModel`   | Load Whisper model weights from disk.          |
//! | `"model.unload"` | `UnloadModel` | Drop model weights from memory.                |
//! | `"inference"`    | `Inference`   | Transcribe f32 PCM audio; returns SRT-style.   |
//!
//! ### `model.load` input JSON
//! ```json
//! { "model_path": "/path/to/model.safetensors", "tokenizer_path": null }
//! ```

use std::sync::Arc;

use tokio::sync::broadcast;

use crate::engine::candle::config::CandleModelLoadConfig;
use crate::engine::candle::whisper::adapter::CandleWhisperEngine;
use crate::scheduler::backend::backend_handler;
use crate::scheduler::backend::protocol::{
    BackendReply, BackendRequest, PeerWorkerCommand, RuntimeControlSignal, WorkerCommand,
};
use crate::scheduler::backend::runner::spawn_workers;
use crate::scheduler::types::Payload;

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
        Self {
            engine,
            bc_tx,
            worker_id,
        }
    }

    /// `lib.load` is a no-op for Candle (statically linked).
    #[on_event(LoadLibrary)]
    async fn on_load_library(&mut self, req: BackendRequest) {
        let _ = req
            .reply_tx
            .send(BackendReply::Value(Payload::Bytes(Arc::from([] as [u8; 0]))));
    }

    /// `lib.reload` is a no-op for Candle (statically linked).
    #[on_event(ReloadLibrary)]
    async fn on_reload_library(&mut self, req: BackendRequest) {
        let _ = req
            .reply_tx
            .send(BackendReply::Value(Payload::Bytes(Arc::from([] as [u8; 0]))));
    }

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

    // ── Runtime / peer control ────────────────────────────────────────────────

    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(&mut self, cmd: PeerWorkerCommand) {
        let PeerWorkerCommand::LoadModel { model_path, .. } = cmd else {
            return;
        };
        if let Some(engine) = self.engine.as_ref() {
            if !engine.is_model_loaded() {
                let engine = engine.clone();
                let result =
                    tokio::task::block_in_place(|| engine.load_model(&model_path, None));
                if let Err(e) = result {
                    tracing::warn!(
                        model_path,
                        error = %e,
                        "candle.whisper worker: broadcast LoadModel failed"
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

    // No-op peer handlers for lib load/reload (Candle has no dylib).
    #[on_peer_control(LoadLibrary)]
    async fn on_peer_load_library(&mut self, _cmd: PeerWorkerCommand) {}

    #[on_peer_control(ReloadLibrary)]
    async fn on_peer_reload_library(&mut self, _cmd: PeerWorkerCommand) {}

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
        let config: CandleModelLoadConfig = match input.to_json() {
            Ok(c) => c,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "invalid model.load config: {e}"
                )));
                return;
            }
        };

        let engine = self.engine.get_or_insert_with(CandleWhisperEngine::new);
        let tok_path = config.tokenizer_path.as_deref();
        let model_path = config.model_path.clone();
        let engine_clone = engine.clone();

        let result = tokio::task::block_in_place(move || {
            engine_clone.load_model(&model_path, tok_path)
        });

        match result {
            Ok(()) => {
                let _ = self
                    .bc_tx
                    .send(WorkerCommand::Peer(PeerWorkerCommand::LoadModel {
                        model_path: config.model_path,
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

    async fn handle_unload_model(
        &mut self,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
        seq_id: u64,
    ) {
        match self.engine.as_ref() {
            Some(engine) => {
                engine.unload();
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
            let _ = reply_tx.send(BackendReply::Error(
                "invalid input: audio samples are empty".into(),
            ));
            return;
        }

        let result = tokio::task::block_in_place(|| engine.inference(&samples));

        match result {
            Ok(text) => {
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from(
                    text.as_bytes(),
                ))));
            }
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "candle.whisper inference failed: {e}"
                )));
            }
        }
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Spawn `count` Candle Whisper backend workers.
pub(crate) fn spawn_backend(
    shared_ingress_rx: crate::scheduler::backend::runner::SharedIngressRx,
    control_tx: broadcast::Sender<WorkerCommand>,
    count: usize,
) {
    spawn_workers(
        shared_ingress_rx,
        control_tx,
        count.max(1),
        |worker_id, bc_tx| CandleWhisperWorker::new(Some(CandleWhisperEngine::new()), bc_tx, worker_id),
    );
}

#[cfg(test)]
mod tests {
    use super::CandleWhisperWorker;
    use crate::scheduler::backend::protocol::RuntimeControlSignal;
    use tokio::sync::broadcast;
    use crate::scheduler::backend::protocol::WorkerCommand;

    fn make_worker() -> CandleWhisperWorker {
        let (bc_tx, _bc_rx) = broadcast::channel::<WorkerCommand>(8);
        CandleWhisperWorker::new(None, bc_tx, 0)
    }

    #[tokio::test]
    async fn global_unload_is_safe_without_engine() {
        let mut worker = make_worker();
        worker
            .apply_runtime_control(RuntimeControlSignal::GlobalUnload { op_id: 1 })
            .await;
        // No panic – test passes.
    }
}
