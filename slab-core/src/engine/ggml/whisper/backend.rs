//! Backend worker adapter for `ggml.whisper`.
//!
//! Defines [`WhisperWorker`] logic for runtime-managed worker loops.
//!
//! # Supported ops
//!
//! | Op string          | Event variant    | Description                                        |
//! |--------------------|------------------|----------------------------------------------------|
//! | `"lib.load"`       | `LoadLibrary`    | Load (skip if already loaded) the whisper dylib.   |
//! | `"lib.reload"`     | `ReloadLibrary`  | Replace the library, discarding current model.     |
//! | `"model.load"`     | `LoadModel`      | Load a model from the pre-loaded library.          |
//! | `"model.unload"`   | `UnloadModel`    | Drop the model handle; call model.load to restore. |
//! | `"inference"`      | `Inference`      | Transcribe audio; input is packed `f32` PCM.       |
//!
//! ### `lib.load` / `lib.reload` input JSON
//! ```json
//! { "lib_path": "/path/to/libwhisper.so" }
//! ```
//!
//! ### `model.load` input JSON
//! ```json
//! { "model_path": "/path/to/model.bin" }
//! ```

use std::sync::Arc;

use tokio::sync::broadcast;

use crate::engine::ggml::config::{LibLoadConfig, ModelLoadConfig};
use crate::engine::ggml::whisper::adapter::GGMLWhisperEngine;
use crate::runtime::backend::backend_handler;
use crate::runtime::backend::protocol::{
    BackendReply, BackendRequest, PeerWorkerCommand, RuntimeControlSignal, WorkerCommand,
};
use crate::runtime::types::Payload;

// ── Worker ────────────────────────────────────────────────────────────────────

/// A single whisper backend worker.
///
/// Each worker **owns** its engine (library handle + model context).  There is
/// no shared mutable state between workers, so no `Mutex` is needed on the
/// context.  When `num_workers > 1` multiple workers are spawned; each worker
/// owns an independent engine forked from the same library handle and manages
/// its own model context independently.
///
/// Workers listen on both the shared `mpsc` ingress queue (competitive 鈥?/// only one worker processes each request) and a `broadcast` channel
/// (fan-out 鈥?every worker receives management commands such as `Unload`).
pub(crate) struct WhisperWorker {
    /// - `None` 鈫?library not loaded.
    /// - `Some(e)` where `e.ctx` is None 鈫?lib loaded, no model.
    /// - `Some(e)` where `e.ctx` is Some 鈫?lib + model loaded.
    engine: Option<GGMLWhisperEngine>,
    /// Broadcast sender shared among all workers so that any worker can
    /// propagate state-change commands (e.g. `Unload`) to all peers.
    bc_tx: broadcast::Sender<WorkerCommand>,
    /// Stable index used to populate `sender_id` when broadcasting.
    worker_id: usize,
}

#[backend_handler]
impl WhisperWorker {
    pub(crate) fn new(
        engine: Option<GGMLWhisperEngine>,
        bc_tx: broadcast::Sender<WorkerCommand>,
        worker_id: usize,
    ) -> Self {
        Self {
            engine,
            bc_tx,
            worker_id,
        }
    }

    #[on_event(LoadLibrary)]
    async fn on_load_library(&mut self, req: BackendRequest) {
        let BackendRequest {
            input,
            broadcast_seq,
            reply_tx,
            ..
        } = req;
        let seq_id = broadcast_seq.unwrap_or(0);
        self.handle_load_library(input, reply_tx, seq_id).await;
    }

    #[on_event(ReloadLibrary)]
    async fn on_reload_library(&mut self, req: BackendRequest) {
        let BackendRequest {
            input,
            broadcast_seq,
            reply_tx,
            ..
        } = req;
        let seq_id = broadcast_seq.unwrap_or(0);
        self.handle_reload_library(input, reply_tx, seq_id).await;
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

    // ── lib.load ──────────────────────────────────────────────────────────────

    async fn handle_load_library(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
        seq_id: u64,
    ) {
        if self.engine.is_some() {
            let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(
                Arc::from([] as [u8; 0]),
            )));
            return;
        }

        let config: LibLoadConfig = match input.to_json() {
            Ok(c) => c,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!("invalid lib.load config: {e}")));
                return;
            }
        };

        match GGMLWhisperEngine::from_path(&config.lib_path) {
            Ok(engine) => {
                self.engine = Some(engine);
                // Broadcast so peer workers also load the same library.
                let _ = self
                    .bc_tx
                    .send(WorkerCommand::Peer(PeerWorkerCommand::LoadLibrary {
                        lib_path: config.lib_path,
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

    // ── lib.reload ────────────────────────────────────────────────────────────

    async fn handle_reload_library(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
        seq_id: u64,
    ) {
        let config: LibLoadConfig = match input.to_json() {
            Ok(c) => c,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "invalid lib.reload config: {e}"
                )));
                return;
            }
        };

        // Drop current engine (lib + model context).
        self.engine = None;

        match GGMLWhisperEngine::from_path(&config.lib_path) {
            Ok(engine) => {
                self.engine = Some(engine);
                // Broadcast so peer workers drop their old engine and reload too.
                let _ = self
                    .bc_tx
                    .send(WorkerCommand::Peer(PeerWorkerCommand::ReloadLibrary {
                        lib_path: config.lib_path,
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

    // ── model.load ────────────────────────────────────────────────────────────

    async fn handle_load_model(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
        seq_id: u64,
    ) {
        let engine = match self.engine.as_mut() {
            Some(e) => e,
            None => {
                let _ = reply_tx.send(BackendReply::Error(
                    "library not loaded; call lib.load first".into(),
                ));
                return;
            }
        };

        let config: ModelLoadConfig = match input.to_json() {
            Ok(c) => c,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "invalid model.load config: {e}"
                )));
                return;
            }
        };

        // Model loading is CPU/I-O bound; use block_in_place on this thread.
        let result = tokio::task::block_in_place(|| {
            use slab_whisper::WhisperContextParameters;
            let params = WhisperContextParameters::default();
            engine.new_context(&config.model_path, params)
        });

        match result {
            Ok(()) => {
                // Broadcast so peer workers also load the same model.
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

    // ── model.unload ──────────────────────────────────────────────────────────

    async fn handle_unload_model(
        &mut self,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
        seq_id: u64,
    ) {
        match self.engine.as_mut() {
            Some(e) => {
                e.unload();
                // Broadcast so every peer worker also drops its context.
                // Ignore errors: no receivers simply means no other workers.
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
                let _ = reply_tx.send(BackendReply::Error(
                    "library not loaded; call lib.load first".into(),
                ));
            }
        }
    }

    // ── inference ─────────────────────────────────────────────────────────────

    async fn handle_inference(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
    ) {
        let engine = match self.engine.as_ref() {
            Some(e) => e,
            None => {
                let _ = reply_tx.send(BackendReply::Error(
                    "whisper backend not ready: library or model not loaded. Call lib.load and model.load first".into(),
                ));
                return;
            }
        };

        let samples = match input.to_f32_arc() {
            Ok(b) => b,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "invalid input for whisper inference: expected f32 PCM audio samples, got: {e}"
                )));
                return;
            }
        };

        if samples.is_empty() {
            let _ = reply_tx.send(BackendReply::Error(
                "invalid input for whisper inference: audio samples are empty".into(),
            ));
            return;
        }

        // Whisper inference is CPU/GPU-bound; use block_in_place so the engine
        // context stays on this thread without needing an additional spawn_blocking.
        let result = tokio::task::block_in_place(|| {
            tracing::debug!(
                sample_count = samples.len(),
                duration_sec = samples.len() as f64 / 16000.0,
                "starting whisper inference"
            );
            engine.inference(&samples)
        });

        match result {
            Err(e) => {
                tracing::error!(error = %e, "whisper inference failed");
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "whisper inference failed: {e}"
                )));
            }
            Ok(entries) => {
                tracing::debug!(segment_count = entries.len(), "whisper inference succeeded");
                let mut out = String::new();
                for entry in entries {
                    if let Some(line) = entry.line {
                        let ts = entry.timespan;
                        out.push_str(&format!(
                            "{} --> {}: {}\n",
                            ts.start.msecs(),
                            ts.end.msecs(),
                            line
                        ));
                    }
                }
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from(
                    out.as_bytes(),
                ))));
            }
        }
    }

    #[on_peer_control(LoadLibrary)]
    async fn on_peer_load_library(&mut self, cmd: PeerWorkerCommand) {
        let PeerWorkerCommand::LoadLibrary { lib_path, .. } = cmd else {
            return;
        };
        if self.engine.is_none() {
            if let Ok(engine) = GGMLWhisperEngine::from_path(&lib_path) {
                self.engine = Some(engine);
            }
        }
    }

    #[on_peer_control(ReloadLibrary)]
    async fn on_peer_reload_library(&mut self, cmd: PeerWorkerCommand) {
        let PeerWorkerCommand::ReloadLibrary { lib_path, .. } = cmd else {
            return;
        };
        self.engine = None;
        if let Ok(engine) = GGMLWhisperEngine::from_path(&lib_path) {
            self.engine = Some(engine);
        }
    }

    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(&mut self, cmd: PeerWorkerCommand) {
        let PeerWorkerCommand::LoadModel { model_path, .. } = cmd else {
            return;
        };
        if let Some(engine) = self.engine.as_mut() {
            if !engine.is_model_loaded() {
                let result = tokio::task::block_in_place(|| {
                    use slab_whisper::WhisperContextParameters;
                    let params = WhisperContextParameters::default();
                    engine.new_context(&model_path, params)
                });
                if let Err(e) = result {
                    tracing::warn!(
                        model_path,
                        error = %e,
                        "whisper worker: broadcast LoadModel failed"
                    );
                }
            }
        }
    }

    #[on_peer_control(Unload)]
    async fn on_peer_unload(&mut self) {
        if let Some(e) = self.engine.as_mut() {
            e.unload();
        }
    }

    #[on_runtime_control(GlobalUnload)]
    #[on_runtime_control(GlobalLoad)]
    async fn apply_runtime_control(&mut self, signal: RuntimeControlSignal) {
        match signal {
            RuntimeControlSignal::GlobalUnload { op_id } => {
                tracing::debug!(op_id, "whisper runtime global unload");
                if let Some(engine) = self.engine.as_mut() {
                    engine.unload();
                }
            }
            RuntimeControlSignal::GlobalLoad { op_id, payload } => {
                let _ = payload;
                tracing::debug!(op_id, "whisper runtime global load pre-cleanup");
                if let Some(engine) = self.engine.as_mut() {
                    engine.unload();
                }
            }
        }
    }

    #[on_control_lagged]
    async fn on_control_lagged(&mut self) {
        if let Some(e) = self.engine.as_mut() {
            e.unload();
        }
    }
}
