//! Backend worker adapter for `candle.llama`.
//!
//! Unlike the GGML backend, the Candle backend does **not** load a dynamic
//! library at runtime – Candle is a statically-linked Rust crate.  The
//! `lib.load` and `lib.reload` operations are therefore accepted but treated
//! as no-ops; every reply is an empty success value so that callers written
//! for the GGML interface continue to work without modification.
//!
//! # Supported ops
//!
//! | Op string            | Event variant     | Description                                     |
//! |----------------------|-------------------|-------------------------------------------------|
//! | `"lib.load"`         | `LoadLibrary`     | No-op (Candle is statically linked).            |
//! | `"lib.reload"`       | `ReloadLibrary`   | No-op.                                          |
//! | `"model.load"`       | `LoadModel`       | Load GGUF model weights from disk.              |
//! | `"model.unload"`     | `UnloadModel`     | Drop model weights from memory.                 |
//! | `"inference"`        | `Inference`       | Unary text generation; input is UTF-8 prompt.   |
//! | `"inference.stream"` | `InferenceStream` | Streaming text generation.                      |
//!
//! ### `model.load` input JSON
//! ```json
//! {
//!   "model_path": "/path/to/model.gguf",
//!   "tokenizer_path": "/path/to/tokenizer.json",
//!   "seed": 0
//! }
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::internal::engine::candle::config::CandleLlamaModelLoadConfig;
use crate::internal::engine::candle::llama::adapter::CandleLlamaEngine;
use crate::internal::engine::candle::llama::errors::SessionId;
use crate::internal::scheduler::backend::protocol::{
    BackendReply, BackendRequest, RuntimeControlSignal, StreamChunk, WorkerCommand,
};
use crate::internal::scheduler::backend::runner::{spawn_runtime_worker, SharedIngressRx};
use crate::internal::scheduler::types::Payload;
use slab_core_macros::backend_handler;
use tokio::sync::broadcast;

// ── Worker ────────────────────────────────────────────────────────────────────

struct CandleLlamaWorker {
    /// Shared engine handle; `None` until `model.load` succeeds.
    engine: Option<Arc<CandleLlamaEngine>>,
    /// Map from caller-supplied `session_key` strings to engine session IDs.
    sessions: HashMap<String, SessionId>,
}

#[backend_handler]
impl CandleLlamaWorker {
    fn new(engine: Option<Arc<CandleLlamaEngine>>) -> Self {
        Self {
            engine,
            sessions: HashMap::new(),
        }
    }

    // ── Event handlers ────────────────────────────────────────────────────────

    /// `lib.load` is a no-op for Candle (statically linked).
    #[on_event(LoadLibrary)]
    async fn on_load_library(&mut self, req: BackendRequest) {
        let _ = req.reply_tx.send(BackendReply::Value(Payload::Bytes(
            Arc::from([] as [u8; 0]),
        )));
    }

    /// `lib.reload` is a no-op for Candle (statically linked).
    #[on_event(ReloadLibrary)]
    async fn on_reload_library(&mut self, req: BackendRequest) {
        let _ = req.reply_tx.send(BackendReply::Value(Payload::Bytes(
            Arc::from([] as [u8; 0]),
        )));
    }

    #[on_event(LoadModel)]
    async fn on_load_model(&mut self, req: BackendRequest) {
        let BackendRequest {
            input, reply_tx, ..
        } = req;
        self.handle_load_model(input, reply_tx).await;
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(&mut self, req: BackendRequest) {
        let BackendRequest { reply_tx, .. } = req;
        self.handle_unload_model(reply_tx).await;
    }

    #[on_event(Inference)]
    async fn on_inference(&mut self, req: BackendRequest) {
        let invocation = match req.invocation() {
            Ok(invocation) => invocation,
            Err(error) => {
                let _ = req.reply_tx.send(BackendReply::Error(error));
                return;
            }
        };
        let BackendRequest {
            input, reply_tx, ..
        } = req;
        let opts = invocation.options.to_serde_value();
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

    #[on_event(InferenceStream)]
    async fn on_inference_stream(&mut self, req: BackendRequest) {
        let invocation = match req.invocation() {
            Ok(invocation) => invocation,
            Err(error) => {
                let _ = req.reply_tx.send(BackendReply::Error(error));
                return;
            }
        };
        let BackendRequest {
            input, reply_tx, ..
        } = req;
        let opts = invocation.options.to_serde_value();
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

    fn cleanup_runtime_state(&mut self) {
        if let Some(engine) = self.engine.as_ref() {
            let _ = engine.unload();
        }
        self.sessions.clear();
    }

    // ── Runtime / peer control ────────────────────────────────────────────────

    #[on_runtime_control(GlobalUnload)]
    #[on_runtime_control(GlobalLoad)]
    async fn apply_runtime_control(&mut self, signal: RuntimeControlSignal) {
        match signal {
            RuntimeControlSignal::GlobalUnload { op_id } => {
                tracing::debug!(op_id, "candle.llama runtime global unload");
                self.cleanup_runtime_state();
            }
            RuntimeControlSignal::GlobalLoad { op_id, payload } => {
                let _ = payload;
                tracing::debug!(op_id, "candle.llama runtime global load pre-cleanup");
                self.cleanup_runtime_state();
            }
        }
    }

    #[on_control_lagged]
    async fn on_control_lagged_cleanup(&mut self) {
        self.cleanup_runtime_state();
    }

    // ── Handler helpers ───────────────────────────────────────────────────────

    async fn handle_load_model(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
    ) {
        let config: CandleLlamaModelLoadConfig = match input.to_json() {
            Ok(c) => c,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "invalid model.load config: {e}"
                )));
                return;
            }
        };

        let engine = Arc::new(CandleLlamaEngine::new(config.seed));

        let tok_path = config.tokenizer_path.as_deref();
        let model_path = config.model_path.clone();
        let seed = config.seed;
        let engine_clone = Arc::clone(&engine);

        let result = tokio::task::block_in_place(move || {
            engine_clone.load_model(&model_path, tok_path, seed)
        });

        match result {
            Ok(()) => {
                // Clear stale sessions from any previously loaded model.
                self.sessions.clear();
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

    async fn handle_unload_model(&mut self, reply_tx: tokio::sync::oneshot::Sender<BackendReply>) {
        match self.engine.as_ref() {
            Some(engine) => {
                let _ = engine.unload();
                self.engine = None;
                self.sessions.clear();
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

        // Resolve or create a session for KV-cache reuse.
        let session_id = if let Some(ref key) = session_key {
            match self.sessions.get(key) {
                Some(&sid) => Some(sid),
                None => match engine.create_session().await {
                    Ok(sid) => {
                        self.sessions.insert(key.clone(), sid);
                        Some(sid)
                    }
                    Err(e) => {
                        let _ = reply_tx.send(BackendReply::Error(format!(
                            "failed to create session: {e}"
                        )));
                        return;
                    }
                },
            }
        } else {
            None
        };

        match engine.inference(&prompt, max_tokens, session_id).await {
            Ok(text) => {
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from(
                    text.as_bytes(),
                ))));
            }
            Err(e) => {
                // Drop the session on error so the next request starts fresh.
                if let Some(key) = session_key {
                    self.sessions.remove(&key);
                }
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
            }
        }
    }

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

        let prompt = match input.to_str() {
            Ok(s) => s.to_owned(),
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!("prompt not str: {e}")));
                return;
            }
        };

        // Resolve or create a session for KV-cache reuse.
        let existing_session_id = if let Some(ref key) = session_key {
            match self.sessions.get(key).copied() {
                Some(sid) => Some(sid),
                None => match engine.create_session().await {
                    Ok(sid) => {
                        self.sessions.insert(key.clone(), sid);
                        Some(sid)
                    }
                    Err(e) => {
                        let _ = reply_tx.send(BackendReply::Error(format!(
                            "failed to create session: {e}"
                        )));
                        return;
                    }
                },
            }
        } else {
            None
        };

        let (proto_tx, proto_rx) = mpsc::channel::<StreamChunk>(64);
        let _ = reply_tx.send(BackendReply::Stream(proto_rx));

        tokio::spawn(async move {
            use crate::internal::engine::candle::llama::errors::StreamChunk as CandleChunk;
            match engine
                .inference_stream(&prompt, max_tokens, existing_session_id)
                .await
            {
                Ok((mut llama_rx, sid)) => {
                    while let Some(chunk) = llama_rx.recv().await {
                        let mapped = match chunk {
                            CandleChunk::Token(t) => StreamChunk::Token(t),
                            CandleChunk::Done => StreamChunk::Done,
                            CandleChunk::Error(e) => StreamChunk::Error(e),
                        };
                        let done = matches!(mapped, StreamChunk::Done | StreamChunk::Error(_));
                        if proto_tx.send(mapped).await.is_err() {
                            break;
                        }
                        if done {
                            break;
                        }
                    }
                    // Only end the engine session when no session_key was provided
                    // (i.e., the session is ephemeral).  Keyed sessions persist for
                    // the lifetime of the worker so they can be reused on subsequent
                    // requests.
                    if existing_session_id.is_none() {
                        let _ = engine.end_session(sid).await;
                    }
                }
                Err(e) => {
                    // If inference_stream fails after a session was created/resolved,
                    // the session entry remains in `self.sessions` (this closure
                    // cannot mutate the worker map).  On the next request the same
                    // session_key will reuse the existing engine session; the engine
                    // tolerates this and will start fresh if the session is empty.
                    let _ = proto_tx.send(StreamChunk::Error(e.to_string())).await;
                }
            }
        });
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Spawn a Candle LLaMA backend worker.
///
/// `engine` may be `None`; the worker will wait for a `model.load` request
/// before processing inference ops.
pub(crate) fn spawn_backend_with_engine(
    shared_ingress_rx: SharedIngressRx,
    control_tx: broadcast::Sender<WorkerCommand>,
    engine: Option<Arc<CandleLlamaEngine>>,
) {
    let worker = CandleLlamaWorker::new(engine);
    spawn_runtime_worker(shared_ingress_rx, control_tx.subscribe(), 0, worker);
}

#[cfg(test)]
mod tests {
    use super::CandleLlamaWorker;
    use crate::internal::scheduler::backend::protocol::RuntimeControlSignal;

    #[tokio::test]
    async fn runtime_global_unload_clears_engine() {
        let mut worker = CandleLlamaWorker::new(None);
        worker
            .apply_runtime_control(RuntimeControlSignal::GlobalUnload { op_id: 1 })
            .await;
        assert!(
            worker.engine.is_none(),
            "global unload should leave engine cleared"
        );
    }

    #[tokio::test]
    async fn runtime_global_load_runs_pre_cleanup() {
        let mut worker = CandleLlamaWorker::new(None);
        use crate::internal::scheduler::types::Payload;
        worker
            .apply_runtime_control(RuntimeControlSignal::GlobalLoad {
                op_id: 2,
                payload: Payload::Json(serde_json::json!({
                    "model_path": "/tmp/model.gguf",
                    "num_workers": 1
                })),
            })
            .await;
        // Engine stays None (no model was actually loaded).
        assert!(worker.engine.is_none());
    }
}
