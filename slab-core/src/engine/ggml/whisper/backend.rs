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
use tokio::sync::mpsc;

use crate::api::Event;
use crate::engine::ggml::whisper::adapter::GGMLWhisperEngine;
use crate::runtime::backend::protocol::{BackendReply, BackendRequest};
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
/// context.  When `backend_capacity > 1` multiple workers are spawned; each
/// worker owns an independent engine forked from the same library handle and
/// manages its own model context independently.
struct WhisperWorker {
    /// - `None` → library not loaded.
    /// - `Some(e)` where `e.ctx` is None → lib loaded, no model.
    /// - `Some(e)` where `e.ctx` is Some → lib + model loaded.
    engine: Option<GGMLWhisperEngine>,
}

impl WhisperWorker {
    fn new(engine: Option<GGMLWhisperEngine>) -> Self {
        Self { engine }
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
                let _ = reply_tx.send(BackendReply::Error(format!("unknown op: {}", op.name)));
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
                let _ = reply_tx.send(BackendReply::Error("model not loaded".into()));
                return;
            }
        };

        let samples = match input.to_f32_arc() {
            Ok(b) => b,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "transcribe input must be f32 PCM bytes: {e}"
                )));
                return;
            }
        };

        // Whisper inference is CPU/GPU-bound; use block_in_place so the engine
        // context stays on this thread without needing an additional spawn_blocking.
        let result =
            tokio::task::block_in_place(|| engine.inference::<std::path::PathBuf>(&samples));

        match result {
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
            }
            Ok(entries) => {
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
/// Spawn `num_workers` whisper backend workers sharing a single ingress channel.
///
/// Used by `api::init` to separate library loading (phase 1) from worker
/// spawning (phase 2) so that no tasks are started if any library fails.
///
/// When `num_workers > 1` each worker receives an independent engine forked
/// from the same library handle (sharing the underlying `Arc<Whisper>`) but
/// with its own empty model context.  This allows `backend_capacity` concurrent
/// inference requests without any lock contention on the model context.
pub(crate) fn spawn_backend_with_engine(
    channel_capacity: usize,
    num_workers: usize,
    engine: Option<GGMLWhisperEngine>,
) -> mpsc::Sender<BackendRequest> {
    let (tx, rx) = mpsc::channel::<BackendRequest>(channel_capacity);
    // Wrap the receiver in an Arc<Mutex<...>> so multiple worker tasks can
    // share a single ingress channel without a separate dispatcher.
    let rx = Arc::new(tokio::sync::Mutex::new(rx));

    let n = num_workers.max(1);
    for _ in 0..n {
        // Each worker gets its own engine fork (same library, empty ctx).
        let worker_engine = engine.as_ref().map(|e| e.fork_library());
        let worker_rx = Arc::clone(&rx);
        tokio::spawn(async move {
            let mut worker = WhisperWorker::new(worker_engine);
            loop {
                let req = { worker_rx.lock().await.recv().await };
                match req {
                    Some(req) => worker.handle(req).await,
                    None => break,
                }
            }
        });
    }

    tx
}
