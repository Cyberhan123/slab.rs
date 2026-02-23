//! Backend worker adapter for `ggml.llama`.
//!
//! Provides [`spawn_backend`] which starts a Tokio task that translates
//! [`BackendRequest`] messages into `GGMLLlamaEngine` API calls.
//!
//! Supported ops
//! - `"model.load"` – load the llama dynamic library and a GGUF model.
//!   Input bytes must be a UTF-8 JSON object:
//!   ```json
//!   { "lib_path": "/path/to/libllama.so",
//!     "model_path": "/path/to/model.gguf",
//!     "num_workers": 1 }
//!   ```
//! - `"generate"` – unary text generation; input is the prompt as UTF-8.
//! - `"generate.stream"` – streaming generation; input is the prompt as UTF-8.
//!
//! Any op called before `"model.load"` returns
//! `BackendReply::Error("model not loaded")`.

use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::mpsc;

use crate::engine::ggml::llama::adapter::GGMLLlamaEngine;
use crate::runtime::backend::protocol::{BackendReply, BackendRequest, StreamChunk};
use crate::runtime::types::Payload;

// ── Load configuration ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LoadConfig {
    lib_path: String,
    model_path: String,
    #[serde(default = "default_workers")]
    num_workers: usize,
}

fn default_workers() -> usize {
    1
}

// ── Worker ────────────────────────────────────────────────────────────────────

struct LlamaWorker {
    /// Non-None after a successful `model.load`.
    engine: Option<Arc<GGMLLlamaEngine>>,
}

impl LlamaWorker {
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
            "model.unload" => self.handle_unload(reply_tx).await,
            "inference" => {
                let max_tokens = serde_json::to_value(&op.options)
                    .unwrap_or(serde_json::Value::Null)
                    .get("max_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(256);
                self.handle_inference(input, max_tokens, reply_tx)
                    .await;
            }
            "inference.stream" => {
                let max_tokens = serde_json::to_value(&op.options)
                    .unwrap_or(serde_json::Value::Null)
                    .get("max_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(256);
                self.handle_inference_stream(input, max_tokens, reply_tx)
                    .await;
            }
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

        if config.num_workers == 0 {
            let _ = reply_tx.send(BackendReply::Error("num_workers must be > 0".into()));
            return;
        }

        let engine = match GGMLLlamaEngine::init(&config.lib_path) {
            Ok(e) => e,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!("init engine: {e}")));
                return;
            }
        };

        use slab_llama::{LlamaContextParams, LlamaModelParams};
        if let Err(e) = engine.load_model_with_workers(
            &config.model_path,
            //TODO: expose these params in the config
            LlamaModelParams::default(),
            LlamaContextParams::default(),
            config.num_workers,
        ) {
            let _ = reply_tx.send(BackendReply::Error(format!("load model: {e}")));
            return;
        }

        self.engine = Some(engine);
        
        let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(
            Arc::from([] as [u8; 0]),
        )));
    }

    async fn handle_unload(&mut self, reply_tx: tokio::sync::oneshot::Sender<BackendReply>) {
        self.engine = None;
        let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(std::sync::Arc::from(&b""[..]))));
    }

    async fn handle_inference(
        &self,
        input: Payload,
        max_tokens: usize,
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
            Ok(s) => s,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!("prompt not str: {e}")));
                return;
            }
        };

        match engine.inference(&prompt, max_tokens, None).await {
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

    async fn handle_inference_stream(
        &self,
        input: Payload,
        max_tokens: usize,
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

        // Create the protocol stream channel and immediately hand it to the caller.
        let (proto_tx, proto_rx) = mpsc::channel::<StreamChunk>(64);
        let _ = reply_tx.send(BackendReply::Stream(proto_rx));

        // Stream inference runs in the background.
        tokio::spawn(async move {
            use crate::engine::ggml::llama::StreamChunk as LlamaChunk;

            match engine.inference_stream(&prompt, max_tokens, None).await {
                Ok((mut llama_rx, sid)) => {
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
                    let _ = engine.end_session(sid).await;
                }
                Err(e) => {
                    let _ = proto_tx.send(StreamChunk::Error(e.to_string())).await;
                }
            }
        });
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Spawn the llama backend worker and return its ingress sender.
///
/// The worker task handles [`BackendRequest`] messages sequentially.
/// It starts with no model loaded; send `op="model.load"` first.
///
/// # Panics
/// Panics if called outside a Tokio runtime.
pub fn spawn_backend(capacity: usize) -> mpsc::Sender<BackendRequest> {
    let (tx, mut rx) = mpsc::channel::<BackendRequest>(capacity);
    tokio::spawn(async move {
        let mut worker = LlamaWorker::new();
        while let Some(req) = rx.recv().await {
            worker.handle(req).await;
        }
    });
    tx
}
