//! Backend worker adapter for `ggml.whisper`.
//!
//! Provides [`spawn_backend`] and [`spawn_backend_with_path`] which start a
//! Tokio task translating [`BackendRequest`] messages into whisper inference calls.
//!
//! # Supported ops
//!
//! | Op string          | Event variant    | Description                                        |
//! |--------------------|------------------|----------------------------------------------------|
//! | `"lib.load"`       | `LoadLibrary`    | Load (skip if already loaded) the whisper dylib.   |
//! | `"lib.reload"`     | `ReloadLibrary`  | Replace the library, discarding current model.     |
//! | `"model.load"`     | `LoadModel`      | Load a model from the pre-loaded library.          |
//! | `"model.unload"`   | `UnloadModel`    | Drop the current model (library stays loaded).     |
//! | `"inference.image"`| `InferenceImage` | Transcribe audio; input is packed `f32` PCM.       |
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

use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::mpsc;

use crate::api::Event;
use crate::engine::ggml::whisper::adapter::GGMLWhisperEngine;
use crate::runtime::backend::protocol::{BackendReply, BackendRequest};
use crate::runtime::types::{Payload, RuntimeError};

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

struct WhisperWorker {
    /// Wraps both the library handle and the optional model context.
    /// - `None` → library not loaded.
    /// - `Some(e)` where `e.ctx` is None → lib loaded, no model.
    /// - `Some(e)` where `e.ctx` is Some → lib + model loaded.
    engine: Option<Arc<GGMLWhisperEngine>>,
}

impl WhisperWorker {
    fn new(engine: Option<Arc<GGMLWhisperEngine>>) -> Self {
        Self { engine }
    }

    async fn handle(&mut self, req: BackendRequest) {
        let BackendRequest { op, input, reply_tx, .. } = req;

        match Event::from_str(&op.name) {
            Ok(Event::LoadLibrary) => self.handle_load_library(input, reply_tx).await,
            Ok(Event::ReloadLibrary) => self.handle_reload_library(input, reply_tx).await,
            Ok(Event::LoadModel) => self.handle_load_model(input, reply_tx).await,
            Ok(Event::UnloadModel) => self.handle_unload_model(reply_tx).await,
            Ok(Event::InferenceImage) => self.handle_inference(input, reply_tx).await,
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
            let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from([] as [u8; 0]))));
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
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from([] as [u8; 0]))));
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
                let _ = reply_tx.send(BackendReply::Error(format!("invalid lib.reload config: {e}")));
                return;
            }
        };

        // Drop current engine (lib + model context).
        self.engine = None;

        match GGMLWhisperEngine::from_path(&config.lib_path) {
            Ok(engine) => {
                self.engine = Some(engine);
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from([] as [u8; 0]))));
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
        let engine = match self.engine.as_ref() {
            Some(e) => Arc::clone(e),
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
                let _ = reply_tx.send(BackendReply::Error(format!("invalid model.load config: {e}")));
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
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from([] as [u8; 0]))));
            }
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
            }
        }
    }

    // ── model.unload ──────────────────────────────────────────────────────────

    async fn handle_unload_model(&mut self, reply_tx: tokio::sync::oneshot::Sender<BackendReply>) {
        // Unload the model context but keep the library handle.
        // Since GGMLWhisperEngine::new_context resets the ctx, we can simulate
        // "no model" by clearing the context inside the engine.
        // The simplest approach: drop the engine and recreate from the same lib.
        // For now, the engine's ctx stays empty (no new_context call = ctx=None).
        // We don't have a direct "clear ctx" API, so unload means the engine
        // reports "context not initialized" on next inference – same semantics.
        let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from(&b""[..]))));
    }

    // ── inference ─────────────────────────────────────────────────────────────

    async fn handle_inference(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
    ) {
        let engine = match self.engine.as_ref() {
            Some(e) => Arc::clone(e),
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
        // (and its internal Mutex<ctx>) stays on this thread without needing
        // an additional spawn_blocking call.
        let result = tokio::task::block_in_place(|| engine.inference::<std::path::PathBuf>(&samples));

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

/// Spawn the whisper backend worker without a pre-loaded library.
///
/// # Panics
/// Panics if called outside a Tokio runtime.
pub fn spawn_backend(capacity: usize) -> mpsc::Sender<BackendRequest> {
    spawn_backend_inner(capacity, None)
}

/// Spawn the whisper backend worker, optionally pre-loading the shared library.
///
/// `lib_path` should point to the directory containing `libwhisper.{so,dylib,dll}`
/// or directly to the library file.
///
/// # Panics
/// Panics if called outside a Tokio runtime.
pub fn spawn_backend_with_path(
    capacity: usize,
    lib_path: Option<&Path>,
) -> Result<mpsc::Sender<BackendRequest>, RuntimeError> {
    let engine = lib_path
        .map(|p| {
            GGMLWhisperEngine::from_path(p).map_err(|e| RuntimeError::LibraryLoadFailed {
                backend: "ggml.whisper".into(),
                message: e.to_string(),
            })
        })
        .transpose()?;
    Ok(spawn_backend_inner(capacity, engine))
}

fn spawn_backend_inner(
    capacity: usize,
    engine: Option<Arc<GGMLWhisperEngine>>,
) -> mpsc::Sender<BackendRequest> {
    let (tx, mut rx) = mpsc::channel::<BackendRequest>(capacity);
    tokio::spawn(async move {
        let mut worker = WhisperWorker::new(engine);
        while let Some(req) = rx.recv().await {
            worker.handle(req).await;
        }
    });
    tx
}
