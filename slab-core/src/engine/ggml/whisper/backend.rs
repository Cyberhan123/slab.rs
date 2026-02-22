//! Backend worker adapter for `ggml.whisper`.
//!
//! Provides [`spawn_backend`] which starts a Tokio task that translates
//! [`BackendRequest`] messages into `GGMLWhisperEngine` API calls.
//!
//! Supported ops
//! - `"model.load"` – load the whisper dynamic library and a model.
//!   Input bytes must be a UTF-8 JSON object:
//!   ```json
//!   { "lib_path": "/path/to/libwhisper.so",
//!     "model_path": "/path/to/model.bin" }
//!   ```
//! - `"transcribe"` – speech-to-text; input is raw little-endian `f32` PCM
//!   samples (16 kHz mono as expected by whisper.cpp).
//!   Returns the transcript as UTF-8 text bytes.
//!
//! Any op called before `"model.load"` returns
//! `BackendReply::Error("model not loaded")`.

use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::mpsc;

use crate::engine::ggml::whisper::adapter::GGMLWhisperEngine;
use crate::runtime::backend::protocol::{BackendReply, BackendRequest};
use crate::runtime::types::Payload;

// ── Load configuration ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LoadConfig {
    lib_path: String,
    model_path: String,
}

// ── Worker ────────────────────────────────────────────────────────────────────

struct WhisperWorker {
    /// Non-None after a successful `model.load`.
    engine: Option<Arc<GGMLWhisperEngine>>,
}

impl WhisperWorker {
    fn new() -> Self {
        Self { engine: None }
    }

    async fn handle(&mut self, req: BackendRequest) {
        let BackendRequest {
            op,
            input,
            reply_tx,
            ..
        } = req;

        match op.name.as_str() {
            "model.load" => self.handle_load(input, reply_tx).await,
            "inference" => self.handle_inference(input, reply_tx).await,
            other => {
                let _ = reply_tx.send(BackendReply::Error(format!("unknown op: {other}")));
            }
        }
    }

    async fn handle_load(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
    ) {
        let config: LoadConfig = match input.to_json() {
            Ok(c) => c,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "invalid model.load config: {e}"
                )));
                return;
            }
        };

        let engine = match GGMLWhisperEngine::init(&config.lib_path) {
            Ok(e) => e,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!("init engine: {e}")));
                return;
            }
        };

        use slab_whisper::WhisperContextParameters;
        let params = WhisperContextParameters::default();
        if let Err(e) = engine.new_context(&config.model_path, params) {
            let _ = reply_tx.send(BackendReply::Error(format!("load model: {e}")));
            return;
        }

        self.engine = Some(engine);
        let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(
            Arc::from([] as [u8; 0]),
        )));
    }

    async fn handle_inference(
        &self,
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

        // Input bytes must be packed little-endian f32 PCM samples.
        let samples = match input.to_f32_arc() {
            Ok(b) => b,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "transcribe input must be f32 PCM bytes: {e}"
                )));

                return;
            }
        };

        // Whisper inference is is CPU/GPU-bound; run in spawn_blocking to avoid blocking
        // the async runtime.
        let result = tokio::task::spawn_blocking(move || {
            // Since inference holds a std::sync::Mutex, it's safe to call here.
            engine.inference::<std::path::PathBuf>(&samples)
        })
        .await;

        match result {
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!("spawn_blocking panic: {e}")));
            }
            Ok(Err(e)) => {
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
            }
            Ok(Ok(entries)) => {
                // Encode each subtitle entry as "start–end: text\n".
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

// ── Public entry point ────────────────────────────────────────────────────────

/// Spawn the whisper backend worker and return its ingress sender.
///
/// The worker task handles [`BackendRequest`] messages sequentially.
/// It starts with no model loaded; send `op="model.load"` first.
///
/// # Panics
/// Panics if called outside a Tokio runtime.
pub fn spawn_backend(capacity: usize) -> mpsc::Sender<BackendRequest> {
    let (tx, mut rx) = mpsc::channel::<BackendRequest>(capacity);
    tokio::spawn(async move {
        let mut worker = WhisperWorker::new();
        while let Some(req) = rx.recv().await {
            worker.handle(req).await;
        }
    });
    tx
}
