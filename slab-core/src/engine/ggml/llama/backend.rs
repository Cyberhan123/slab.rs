//! Backend worker adapter for `ggml.llama`.
//!
//! Provides [`spawn_backend`] and [`spawn_backend_with_path`] which start a
//! Tokio task translating [`BackendRequest`] messages into llama inference calls.
//!
//! # Supported ops
//!
//! | Op string            | Event variant    | Description                                    |
//! |----------------------|------------------|------------------------------------------------|
//! | `"lib.load"`         | `LoadLibrary`    | Load (skip if already loaded) the llama dylib. |
//! | `"lib.reload"`       | `ReloadLibrary`  | Replace the library, discarding current model. |
//! | `"model.load"`       | `LoadModel`      | Load a GGUF model from the pre-loaded library. |
//! | `"model.unload"`     | `UnloadModel`    | Drop the model and library handle; call lib.load + model.load to restore. |
//! | `"inference"`        | `Inference`      | Unary text generation; input is UTF-8 prompt.  |
//! | `"inference.stream"` | `InferenceStream`| Streaming text generation.                     |
//!
//! ### `lib.load` / `lib.reload` input JSON
//! ```json
//! { "lib_path": "/path/to/libllama.so" }
//! ```
//!
//! ### `model.load` input JSON
//! ```json
//! { "model_path": "/path/to/model.gguf", "num_workers": 1 }
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::mpsc;

use crate::api::Event;
use crate::engine::ggml::llama::adapter::GGMLLlamaEngine;
use crate::engine::ggml::llama::errors::SessionId;
use crate::runtime::backend::protocol::{BackendReply, BackendRequest, StreamChunk};
use crate::runtime::types::{Payload, RuntimeError};

// ── Configurations ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LibLoadConfig {
    lib_path: String,
}

#[derive(Deserialize)]
struct ModelLoadConfig {
    model_path: String,
    #[serde(default = "default_workers")]
    num_workers: usize,
}

fn default_workers() -> usize {
    1
}

// ── Worker ────────────────────────────────────────────────────────────────────

struct LlamaWorker {
    /// The engine: wraps both the library handle and inference workers.
    /// - `None` → library not loaded.
    /// - `Some(e)` where `e.inference_engine` is None → lib loaded, no model.
    /// - `Some(e)` where `e.inference_engine` is Some → lib + model loaded.
    engine: Option<Arc<GGMLLlamaEngine>>,
    /// Maps caller-provided session keys to engine-internal session IDs.
    sessions: HashMap<String, SessionId>,
}

impl LlamaWorker {
    fn new(engine: Option<Arc<GGMLLlamaEngine>>) -> Self {
        Self {
            engine,
            sessions: HashMap::new(),
        }
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
            Ok(Event::LoadModel) => {
                self.handle_load_model(input, reply_tx).await;
            }
            Ok(Event::UnloadModel) => self.handle_unload_model(reply_tx).await,
            Ok(Event::Inference) => {
                let opts = op.options.to_serde_value();
                let max_tokens = opts
                    .get("max_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(256);
                let session_key = opts
                    .get("session_key")
                    .and_then(|s| s.as_str())
                    .map(str::to_owned);
                self.handle_inference(input, max_tokens, session_key, reply_tx)
                    .await;
            }
            Ok(Event::InferenceStream) => {
                let opts = op.options.to_serde_value();
                let max_tokens = opts
                    .get("max_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(256);
                let session_key = opts
                    .get("session_key")
                    .and_then(|s| s.as_str())
                    .map(str::to_owned);
                self.handle_inference_stream(input, max_tokens, session_key, reply_tx)
                    .await;
            }
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
            // Library already loaded; skip silently.
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

        match GGMLLlamaEngine::from_path(&config.lib_path) {
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

        // Drop current engine (releases model and inference OS threads).
        self.engine = None;
        self.sessions.clear();

        match GGMLLlamaEngine::from_path(&config.lib_path) {
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
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "invalid model.load config: {e}"
                )));
                return;
            }
        };

        if config.num_workers == 0 {
            let _ = reply_tx.send(BackendReply::Error("num_workers must be > 0".into()));
            return;
        }

        // Reset sessions (old model is being replaced).
        self.sessions.clear();

        // Model loading is CPU/blocking; use block_in_place to avoid stalling
        // the async runtime without the Send constraint of spawn_blocking.
        let result = tokio::task::block_in_place(|| {
            use slab_llama::{LlamaContextParams, LlamaModelParams};
            engine.load_model_with_workers(
                &config.model_path,
                LlamaModelParams::default(),
                LlamaContextParams::default(),
                config.num_workers,
            )
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
        // Drop the current GGMLLlamaEngine instance (and thus the loaded model and
        // its associated OS worker threads) by clearing our handle to it.
        // Subsequent inference calls will observe `self.engine == None` and return
        // "model not loaded" until lib.load + model.load are called again.
        //
        // Note: this also releases the dynamic library handle held inside the engine.
        // Call lib.load first before model.load to restore full functionality.
        self.sessions.clear();
        self.engine = None;
        let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from(&b""[..]))));
    }

    // ── inference ─────────────────────────────────────────────────────────────

    async fn handle_inference(
        &mut self,
        input: Payload,
        max_tokens: usize,
        session_key: Option<String>,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
    ) {
        let engine = match self.engine.as_ref() {
            Some(e) => Arc::clone(e),
            None => {
                let _ = reply_tx.send(BackendReply::Error("model not loaded".into()));
                return;
            }
        };

        let prompt = match input.to_str() {
            Ok(s) => s.to_owned(),
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!("prompt not str: {e}")));
                return;
            }
        };

        let llama_sid = session_key
            .as_ref()
            .and_then(|k| self.sessions.get(k))
            .copied();

        match engine.inference(&prompt, max_tokens, llama_sid).await {
            Ok(text) => {
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from(
                    text.as_bytes(),
                ))));
            }
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
            }
        }
    }

    // ── inference.stream ──────────────────────────────────────────────────────

    async fn handle_inference_stream(
        &mut self,
        input: Payload,
        max_tokens: usize,
        session_key: Option<String>,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
    ) {
        let engine = match self.engine.as_ref() {
            Some(e) => Arc::clone(e),
            None => {
                let _ = reply_tx.send(BackendReply::Error("model not loaded".into()));
                return;
            }
        };

        let prompt = match input.to_str_arc() {
            Ok(s) => s,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!("prompt not str: {e}")));
                return;
            }
        };

        let llama_sid = session_key
            .as_ref()
            .and_then(|k| self.sessions.get(k))
            .copied();

        let (proto_tx, proto_rx) = mpsc::channel::<StreamChunk>(64);
        let _ = reply_tx.send(BackendReply::Stream(proto_rx));

        let (sid_tx, sid_rx) = tokio::sync::oneshot::channel::<(String, SessionId)>();

        tokio::spawn(async move {
            use crate::engine::ggml::llama::StreamChunk as LlamaChunk;

            match engine
                .inference_stream(&prompt, max_tokens, llama_sid)
                .await
            {
                Ok((mut llama_rx, new_sid)) => {
                    while let Some(chunk) = llama_rx.recv().await {
                        let mapped = match chunk {
                            LlamaChunk::Token(t) => StreamChunk::Token(t),
                            LlamaChunk::Done => StreamChunk::Done,
                            LlamaChunk::Error(e) => StreamChunk::Error(e),
                        };
                        let is_done = matches!(mapped, StreamChunk::Done);
                        let is_err = matches!(mapped, StreamChunk::Error(_));
                        if proto_tx.send(mapped).await.is_err() || is_done || is_err {
                            break;
                        }
                    }
                    if let Some(key) = session_key {
                        let _ = sid_tx.send((key, new_sid));
                        return;
                    }
                    let _ = engine.end_session(new_sid).await;
                }
                Err(e) => {
                    let _ = proto_tx.send(StreamChunk::Error(e.to_string())).await;
                }
            }
        });

        if let Ok((key, new_sid)) = sid_rx.await {
            self.sessions.insert(key, new_sid);
        }
    }
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Spawn the llama backend worker without a pre-loaded library.
///
/// # Panics
/// Panics if called outside a Tokio runtime.
pub fn spawn_backend(capacity: usize) -> mpsc::Sender<BackendRequest> {
    spawn_backend_inner(capacity, None)
}

/// Spawn the llama backend worker, optionally pre-loading the shared library.
///
/// `lib_path` should point to the directory containing `libllama.{so,dylib,dll}`
/// or directly to the library file.
///
/// # Panics
/// Panics if called outside a Tokio runtime.
pub fn spawn_backend_with_path(
    capacity: usize,
    lib_path: Option<&Path>,
) -> Result<mpsc::Sender<BackendRequest>, RuntimeError> {
    let engine = lib_path
        .map(|path| {
            GGMLLlamaEngine::from_path(path).map_err(|e| RuntimeError::LibraryLoadFailed {
                backend: "ggml.llama".into(),
                message: e.to_string(),
            })
        })
        .transpose()?;
    Ok(spawn_backend_inner(capacity, engine))
}

fn spawn_backend_inner(
    capacity: usize,
    engine: Option<Arc<GGMLLlamaEngine>>,
) -> mpsc::Sender<BackendRequest> {
    let (tx, mut rx) = mpsc::channel::<BackendRequest>(capacity);
    tokio::spawn(async move {
        let mut worker = LlamaWorker::new(engine);
        while let Some(req) = rx.recv().await {
            worker.handle(req).await;
        }
    });
    tx
}

/// Spawn a llama backend worker with a pre-loaded engine handle.
///
/// Used by `api::init` to separate library loading (phase 1) from worker
/// spawning (phase 2) so that no tasks are started if any library fails.
pub(crate) fn spawn_backend_with_engine(
    capacity: usize,
    engine: Option<Arc<GGMLLlamaEngine>>,
) -> mpsc::Sender<BackendRequest> {
    spawn_backend_inner(capacity, engine)
}
