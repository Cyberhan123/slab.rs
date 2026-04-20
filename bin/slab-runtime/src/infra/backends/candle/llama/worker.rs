//! Backend worker for `candle.llama`.
//!
//! Unlike the GGML backend, the Candle backend does **not** load a dynamic
//! library at runtime – Candle is a statically-linked Rust crate.
//!
//! # Supported ops
//!
//! | Op string            | Event variant     | Description                                     |
//! |----------------------|-------------------|-------------------------------------------------|
//! | `"model.load"`       | `LoadModel`       | Load GGUF model weights from disk.              |
//! | `"model.unload"`     | `UnloadModel`     | Drop model weights from memory.                 |
//! | `"inference"`        | `Inference`       | Unary text generation; input is UTF-8 prompt.   |
//! | `"inference.stream"` | `InferenceStream` | Streaming text generation.                      |
//!
//! ### `model.load` input payload
//! Uses typed runtime-owned payloads inside `slab-runtime`.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;

use super::contract::{
    CandleLlamaLoadConfig, TextGenerationOptions, TextGenerationResponse, TextGenerationStreamEvent,
};
use super::engine::CandleLlamaEngine;
use super::error::{CandleLlamaWorkerError, SessionId};
use slab_runtime_core::backend::{
    ControlOpId, Input, Options, StreamChunk, StreamHandle, Typed, WorkerCommand,
};
use slab_runtime_core::backend::{SharedIngressRx, spawn_runtime_worker};
use slab_runtime_macros::backend_handler;
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
        Self { engine, sessions: HashMap::new() }
    }

    // ── Event handlers ────────────────────────────────────────────────────────

    #[on_event(LoadModel)]
    async fn on_load_model(
        &mut self,
        config: Input<CandleLlamaLoadConfig>,
    ) -> Result<(), CandleLlamaWorkerError> {
        self.handle_load_model(config.0).await
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(&mut self) -> Result<(), CandleLlamaWorkerError> {
        self.handle_unload_model().await
    }

    #[on_event(Inference)]
    async fn on_inference(
        &mut self,
        prompt: String,
        options: Options<TextGenerationOptions>,
    ) -> Result<Typed<TextGenerationResponse>, CandleLlamaWorkerError> {
        let max_tokens =
            options.0.max_tokens.and_then(|value| usize::try_from(value).ok()).unwrap_or(256);
        let session_key = options.0.session_key;
        self.handle_inference(prompt, max_tokens, session_key).await
    }

    #[on_event(InferenceStream)]
    async fn on_inference_stream(
        &mut self,
        prompt: String,
        options: Options<TextGenerationOptions>,
    ) -> Result<StreamHandle, CandleLlamaWorkerError> {
        let max_tokens =
            options.0.max_tokens.and_then(|value| usize::try_from(value).ok()).unwrap_or(256);
        let session_key = options.0.session_key;
        self.handle_inference_stream(prompt, max_tokens, session_key).await
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
    async fn apply_runtime_control(
        &mut self,
        op_id: ControlOpId,
    ) -> Result<(), CandleLlamaWorkerError> {
        tracing::debug!(op_id = op_id.0, "candle.llama runtime control pre-cleanup");
        self.cleanup_runtime_state();
        Ok(())
    }

    #[on_control_lagged]
    async fn on_control_lagged_cleanup(&mut self) -> Result<(), CandleLlamaWorkerError> {
        self.cleanup_runtime_state();
        Ok(())
    }

    // ── Handler helpers ───────────────────────────────────────────────────────

    async fn handle_load_model(
        &mut self,
        config: CandleLlamaLoadConfig,
    ) -> Result<(), CandleLlamaWorkerError> {
        let engine = Arc::new(CandleLlamaEngine::new(config.seed));

        let tokenizer_path = config.tokenizer_path;
        let model_path = config.model_path;
        let seed = config.seed;
        let engine_clone = Arc::clone(&engine);

        let result = tokio::task::block_in_place(move || {
            engine_clone.load_model(&model_path, tokenizer_path.as_deref(), seed)
        });

        match result {
            Ok(()) => {
                // Clear stale sessions from any previously loaded model.
                self.sessions.clear();
                self.engine = Some(engine);
                Ok(())
            }
            Err(error) => Err(CandleLlamaWorkerError::load(error.to_string())),
        }
    }

    async fn handle_unload_model(&mut self) -> Result<(), CandleLlamaWorkerError> {
        match self.engine.as_ref() {
            Some(engine) => {
                let _ = engine.unload();
                self.engine = None;
                self.sessions.clear();
                Ok(())
            }
            None => Err(CandleLlamaWorkerError::unload("model not loaded")),
        }
    }

    async fn handle_inference(
        &mut self,
        prompt: String,
        max_tokens: usize,
        session_key: Option<String>,
    ) -> Result<Typed<TextGenerationResponse>, CandleLlamaWorkerError> {
        let engine = match self.engine.as_ref() {
            Some(e) => Arc::clone(e),
            None => return Err(CandleLlamaWorkerError::inference("model not loaded")),
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
                    Err(error) => {
                        return Err(CandleLlamaWorkerError::inference(format!(
                            "failed to create session: {error}"
                        )));
                    }
                },
            }
        } else {
            None
        };

        match engine.inference(&prompt, max_tokens, session_id).await {
            Ok(text) => Ok(Typed(TextGenerationResponse {
                text,
                finish_reason: Some("stop".to_owned()),
                ..Default::default()
            })),
            Err(e) => {
                // Drop the session on error so the next request starts fresh.
                if let Some(key) = session_key {
                    self.sessions.remove(&key);
                }
                Err(CandleLlamaWorkerError::inference(e.to_string()))
            }
        }
    }

    async fn handle_inference_stream(
        &mut self,
        prompt: String,
        max_tokens: usize,
        session_key: Option<String>,
    ) -> Result<StreamHandle, CandleLlamaWorkerError> {
        let engine = match self.engine.as_ref() {
            Some(e) => Arc::clone(e),
            None => return Err(CandleLlamaWorkerError::inference("model not loaded")),
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
                    Err(error) => {
                        return Err(CandleLlamaWorkerError::inference(format!(
                            "failed to create session: {error}"
                        )));
                    }
                },
            }
        } else {
            None
        };

        let (proto_tx, proto_rx) = mpsc::channel::<StreamChunk>(64);

        tokio::spawn(async move {
            use super::error::StreamChunk as CandleChunk;
            match engine.inference_stream(&prompt, max_tokens, existing_session_id).await {
                Ok((mut llama_rx, sid)) => {
                    while let Some(chunk) = llama_rx.recv().await {
                        let mapped = match chunk {
                            CandleChunk::Token(t) => StreamChunk::Token(t),
                            CandleChunk::Done => StreamChunk::Json(
                                serde_json::to_value(TextGenerationStreamEvent {
                                    done: Some(true),
                                    finish_reason: Some("stop".to_owned()),
                                    ..Default::default()
                                })
                                .expect("candle llama terminal stream event should serialize"),
                            ),
                            CandleChunk::Error(e) => StreamChunk::Error(e),
                        };
                        let done = matches!(mapped, StreamChunk::Json(_) | StreamChunk::Error(_));
                        if proto_tx.send(mapped).await.is_err() {
                            break;
                        }
                        if done {
                            break;
                        }
                    }
                    let _ = proto_tx.send(StreamChunk::Done).await;
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

        Ok(proto_rx)
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Spawn a Candle LLaMA backend worker.
///
/// `engine` may be `None`; the worker will wait for a `model.load` request
/// before processing inference ops.
pub fn spawn_backend_with_engine(
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
    use slab_runtime_core::backend::ControlOpId;

    #[tokio::test]
    async fn runtime_global_unload_clears_engine() {
        let mut worker = CandleLlamaWorker::new(None);
        worker.apply_runtime_control(ControlOpId(1)).await.expect("control cleanup should succeed");
        assert!(worker.engine.is_none(), "global unload should leave engine cleared");
    }

    #[tokio::test]
    async fn runtime_global_load_runs_pre_cleanup() {
        let mut worker = CandleLlamaWorker::new(None);
        worker.apply_runtime_control(ControlOpId(2)).await.expect("control cleanup should succeed");
        // Engine stays None (no model was actually loaded).
        assert!(worker.engine.is_none());
    }
}
