//! Backend worker adapter for `ggml.whisper`.
//!
//! Provides [`spawn_backend_with_engine`] which starts one or more Tokio tasks
//! translating [`BackendRequest`] messages into whisper inference calls.
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

use std::str::FromStr;
use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::{broadcast, mpsc};

use crate::api::Event;
use crate::engine::ggml::whisper::adapter::GGMLWhisperEngine;
use crate::runtime::backend::protocol::{BackendReply, BackendRequest, WorkerCommand};
use crate::runtime::types::Payload;

// ── Configurations ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LibLoadConfig {
    lib_path: String,
}

#[derive(Deserialize)]
struct ModelLoadConfig {
    model_path: String,
}

// ── Worker ────────────────────────────────────────────────────────────────────

/// A single whisper backend worker.
///
/// Each worker **owns** its engine (library handle + model context).  There is
/// no shared mutable state between workers, so no `Mutex` is needed on the
/// context.  When `num_workers > 1` multiple workers are spawned; each worker
/// owns an independent engine forked from the same library handle and manages
/// its own model context independently.
///
/// Workers listen on both the shared `mpsc` ingress queue (competitive –
/// only one worker processes each request) and a `broadcast` channel
/// (fan-out – every worker receives management commands such as `Unload`).
struct WhisperWorker {
    /// - `None` → library not loaded.
    /// - `Some(e)` where `e.ctx` is None → lib loaded, no model.
    /// - `Some(e)` where `e.ctx` is Some → lib + model loaded.
    engine: Option<GGMLWhisperEngine>,
    /// Broadcast sender shared among all workers so that any worker can
    /// propagate state-change commands (e.g. `Unload`) to all peers.
    bc_tx: broadcast::Sender<WorkerCommand>,
    /// Stable index used to populate `sender_id` when broadcasting and to
    /// filter out self-sent commands in the broadcast receive arm.
    worker_id: usize,
}

impl WhisperWorker {
    fn new(engine: Option<GGMLWhisperEngine>, bc_tx: broadcast::Sender<WorkerCommand>, worker_id: usize) -> Self {
        Self { engine, bc_tx, worker_id }
    }

    async fn handle(&mut self, req: BackendRequest) {
        let BackendRequest {
            op,
            input,
            reply_tx,
            ..
        } = req;

        match Event::from_str(&op.name) {
            Ok(Event::LoadLibrary) => self.handle_load_library(input, reply_tx).await,
            Ok(Event::ReloadLibrary) => self.handle_reload_library(input, reply_tx).await,
            Ok(Event::LoadModel) => self.handle_load_model(input, reply_tx).await,
            Ok(Event::UnloadModel) => self.handle_unload_model(reply_tx).await,
            Ok(Event::Inference) => self.handle_inference(input, reply_tx).await,
            Ok(_) | Err(_) => {
                // Handle the is_ready operation for health checks
                if op.name == "is_ready" {
                    let ready = self.engine.as_ref().map_or(false, |e| e.is_model_loaded());
                    let _ = reply_tx.send(BackendReply::Value(Payload::Json(
                        serde_json::json!({ "ready": ready }),
                    )));
                } else {
                    let _ = reply_tx.send(BackendReply::Error(format!("unknown op: {}", op.name)));
                }
            }
        }
    }

    // ── lib.load ──────────────────────────────────────────────────────────────

    async fn handle_load_library(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
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
                    .send(WorkerCommand::LoadLibrary { lib_path: config.lib_path, sender_id: self.worker_id });
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
                    .send(WorkerCommand::ReloadLibrary { lib_path: config.lib_path, sender_id: self.worker_id });
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
                    .send(WorkerCommand::LoadModel { model_path: config.model_path, sender_id: self.worker_id });
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

    async fn handle_unload_model(&mut self, reply_tx: tokio::sync::oneshot::Sender<BackendReply>) {
        match self.engine.as_mut() {
            Some(e) => {
                e.unload();
                // Broadcast so every peer worker also drops its context.
                // Ignore errors: no receivers simply means no other workers.
                let _ = self.bc_tx.send(WorkerCommand::Unload { sender_id: self.worker_id });
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
}

// ── Public entry points ───────────────────────────────────────────────────────
/// Spawn one or more whisper backend workers sharing a pre-loaded engine handle.
///
/// # Returns
///
/// A pair of:
/// - `mpsc::Sender<BackendRequest>` – the ingress queue; inference requests are
///   dispatched here in *competitive* mode (exactly one worker handles each
///   message).
/// - `broadcast::Sender<WorkerCommand>` – the management broadcast channel;
///   every active worker receives each command (e.g. `Unload`).
///
/// # Multi-worker model
///
/// `num_workers` tasks share a single `Arc<Mutex<Receiver>>` so that only
/// one worker processes each inference request.  Every worker also subscribes
/// to the broadcast channel and reacts to management commands independently,
/// ensuring consistent state across all workers.
///
/// Worker 0 receives the original `engine` handle (library loaded, no model
/// context).  Workers 1..n each receive a *forked* engine that shares the
/// same library `Arc` but starts with an empty context.
pub(crate) fn spawn_backend_with_engine(
    channel_capacity: usize,
    num_workers: usize,
    engine: Option<GGMLWhisperEngine>,
) -> (mpsc::Sender<BackendRequest>, broadcast::Sender<WorkerCommand>) {
    let (tx, rx) = mpsc::channel::<BackendRequest>(channel_capacity);
    // Broadcast capacity of 16 is generous for low-frequency management commands.
    let (bc_tx, _) = broadcast::channel::<WorkerCommand>(16);

    let num_workers = num_workers.max(1);
    // Wrap the receiver so multiple workers can compete for messages.
    let rx = Arc::new(tokio::sync::Mutex::new(rx));

    // Build per-worker engine handles: worker 0 gets the original, the rest
    // get library-sharing forks with an empty context.
    let mut worker_engines: Vec<Option<GGMLWhisperEngine>> = (1..num_workers)
        .map(|_| engine.as_ref().map(|e| e.fork_library()))
        .collect();
    worker_engines.insert(0, engine);

    for (worker_id, worker_engine) in worker_engines.into_iter().enumerate() {
        let rx = Arc::clone(&rx);
        let mut bc_rx = bc_tx.subscribe();
        let mut worker = WhisperWorker::new(worker_engine, bc_tx.clone(), worker_id);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased; // prioritize management commands over inference

                    // ── Broadcast arm: management commands ────────────────
                    cmd = bc_rx.recv() => {
                        match cmd {
                            Ok(WorkerCommand::LoadLibrary { lib_path, sender_id }) => {
                                // Skip commands sent by this worker (self-echo).
                                // Also skip if library is already loaded (idempotent).
                                if sender_id != worker.worker_id && worker.engine.is_none() {
                                    if let Ok(engine) = GGMLWhisperEngine::from_path(&lib_path) {
                                        worker.engine = Some(engine);
                                    }
                                }
                            }
                            Ok(WorkerCommand::ReloadLibrary { lib_path, sender_id }) => {
                                // Skip the self-echo: the broadcasting worker already
                                // completed the reload while handling the mpsc request.
                                // Processing it again would drop the freshly-loaded
                                // engine and trigger an unnecessary second FFI load.
                                if sender_id != worker.worker_id {
                                    worker.engine = None;
                                    if let Ok(engine) = GGMLWhisperEngine::from_path(&lib_path) {
                                        worker.engine = Some(engine);
                                    }
                                }
                            }
                            Ok(WorkerCommand::LoadModel { model_path, sender_id }) => {
                                // Skip commands sent by this worker (self-echo).
                                // Also skip if model is already loaded (idempotent).
                                if sender_id != worker.worker_id {
                                    if let Some(engine) = worker.engine.as_mut() {
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
                            }
                            Ok(WorkerCommand::Unload { sender_id }) => {
                                // Skip the self-echo: the broadcasting worker already
                                // called unload() while handling the mpsc request.
                                // Calling it again on the same context is unsafe.
                                if sender_id != worker.worker_id {
                                    if let Some(e) = worker.engine.as_mut() {
                                        e.unload();
                                    }
                                }
                            }
                            // Sender dropped → no more commands; exit.
                            Err(broadcast::error::RecvError::Closed) => break,
                            // Fell behind; missed one or more messages.  To avoid
                            // keeping stale context (e.g. if an Unload was missed),
                            // conservatively unload so the worker returns to a
                            // known-safe state.
                            Err(broadcast::error::RecvError::Lagged(_)) => {
                                if let Some(e) = worker.engine.as_mut() {
                                    e.unload();
                                }
                            }
                        }
                    }

                    // ── mpsc arm: competitive inference tasks ─────────────
                    req = async {
                        let mut lock = rx.lock().await;
                        lock.recv().await
                    } => {
                        match req {
                            Some(req) => worker.handle(req).await,
                            // All senders dropped → shut down this worker.
                            None => break,
                        }
                    }
                }
            }
        });
    }

    (tx, bc_tx)
}
