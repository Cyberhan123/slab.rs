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
//! | `"model.unload"`     | `UnloadModel`    | Drop the model handle; call model.load to restore. |
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
//! { "model_path": "/path/to/model.gguf", "num_workers": 1, "context_length": 4096 }
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;
use slab_llama::ChatMessage as LlamaChatMessage;
use tokio::sync::{broadcast, mpsc};

use crate::engine::ggml::config::LibLoadConfig;
use crate::engine::ggml::llama::adapter::GGMLLlamaEngine;
use crate::engine::ggml::llama::errors::SessionId;
use crate::scheduler::backend::backend_handler;
use crate::scheduler::backend::protocol::{
    BackendReply, BackendRequest, RuntimeControlSignal, StreamChunk, WorkerCommand,
};
use crate::scheduler::backend::runner::{spawn_runtime_worker, SharedIngressRx};
use crate::scheduler::types::Payload;

// ── Configurations ────────────────────────────────────────────────────────────

/// Extended model-load config for llama; includes workers and context length.
#[derive(Deserialize)]
struct LlamaModelLoadConfig {
    model_path: String,
    #[serde(default = "default_workers")]
    num_workers: usize,
    #[serde(default)]
    context_length: u32,
}

fn default_workers() -> usize {
    1
}

struct ParsedChatPrompt {
    messages: Vec<LlamaChatMessage>,
    add_assistant_prompt: bool,
}

fn parse_role_prefixed_chat_prompt(prompt: &str) -> Option<ParsedChatPrompt> {
    // Only attempt template application for the legacy "Role: content" prompt shape.
    if !(prompt.contains("User:") || prompt.contains("Assistant:") || prompt.contains("System:")) {
        return None;
    }

    let mut messages: Vec<LlamaChatMessage> = Vec::new();
    for raw_line in prompt.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            continue;
        }
        let (raw_role, raw_content) = line.split_once(':')?;
        let role = raw_role.trim().to_ascii_lowercase();
        if !matches!(role.as_str(), "system" | "user" | "assistant") {
            return None;
        }
        messages.push(LlamaChatMessage {
            role,
            content: raw_content.trim_start().to_owned(),
        });
    }

    if messages.is_empty() {
        return None;
    }

    let mut add_assistant_prompt = false;
    if let Some(last) = messages.last() {
        if last.role == "assistant" && last.content.is_empty() {
            let _ = messages.pop();
            add_assistant_prompt = true;
        } else if last.role != "assistant" {
            add_assistant_prompt = true;
        }
    }

    if messages.is_empty() && !add_assistant_prompt {
        return None;
    }

    Some(ParsedChatPrompt {
        messages,
        add_assistant_prompt,
    })
}

// ── Worker ────────────────────────────────────────────────────────────────────

struct LlamaWorker {
    /// The engine: wraps both the library handle and inference workers.
    /// - `None` → library not loaded.
    /// - `Some(e)` where `e.inference_engine` is None → lib loaded, no model.
    /// - `Some(e)` where `e.inference_engine` is Some → lib + model loaded.
    engine: Option<Arc<GGMLLlamaEngine>>,
    /// Maps caller-provided session keys to engine-internal session state.
    sessions: HashMap<String, SessionBinding>,
}

#[derive(Debug, Clone)]
struct SessionBinding {
    sid: SessionId,
    /// Prefix already committed in KV for this session (prompt + generated text).
    cached_prompt: String,
}

#[derive(Debug)]
struct PreparedSession {
    key: Option<String>,
    sid: Option<SessionId>,
    delta_prompt: String,
    full_prompt: String,
}

#[derive(Debug)]
enum SessionUpdate {
    Keep {
        key: String,
        sid: SessionId,
        cached_prompt: String,
    },
    Drop {
        key: String,
        sid: SessionId,
    },
}

#[backend_handler]
impl LlamaWorker {
    fn new(engine: Option<Arc<GGMLLlamaEngine>>) -> Self {
        Self {
            engine,
            sessions: HashMap::new(),
        }
    }

    #[on_event(LoadLibrary)]
    async fn on_load_library(&mut self, req: BackendRequest) {
        let BackendRequest {
            input, reply_tx, ..
        } = req;
        self.handle_load_library(input, reply_tx).await;
    }

    #[on_event(ReloadLibrary)]
    async fn on_reload_library(&mut self, req: BackendRequest) {
        let BackendRequest {
            input, reply_tx, ..
        } = req;
        self.handle_reload_library(input, reply_tx).await;
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
        let BackendRequest {
            op,
            input,
            reply_tx,
            ..
        } = req;
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

    #[on_event(InferenceStream)]
    async fn on_inference_stream(&mut self, req: BackendRequest) {
        let BackendRequest {
            op,
            input,
            reply_tx,
            ..
        } = req;
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

    fn cleanup_runtime_state(&mut self) {
        if let Some(engine) = self.engine.as_ref() {
            let _ = engine.unload();
        }
        self.sessions.clear();
    }

    async fn prepare_session(
        &mut self,
        engine: &GGMLLlamaEngine,
        session_key: Option<&str>,
        full_prompt: String,
    ) -> Result<PreparedSession, String> {
        let Some(raw_key) = session_key else {
            return Ok(PreparedSession {
                key: None,
                sid: None,
                delta_prompt: full_prompt.clone(),
                full_prompt,
            });
        };

        let key = raw_key.to_owned();
        let mut sid: Option<SessionId> = None;
        let mut delta_prompt = full_prompt.clone();
        let mut stale_sid: Option<SessionId> = None;

        if let Some(binding) = self.sessions.get(&key) {
            if let Some(delta) = full_prompt.strip_prefix(&binding.cached_prompt) {
                if delta.is_empty() {
                    // No incremental input was added; reset session so regenerate-style
                    // requests still work instead of stalling on an empty append.
                    stale_sid = Some(binding.sid);
                } else {
                    sid = Some(binding.sid);
                    delta_prompt = delta.to_owned();
                }
            } else {
                // Caller-supplied history diverged from cached prefix; reset session.
                stale_sid = Some(binding.sid);
            }
        }

        if let Some(old_sid) = stale_sid {
            self.sessions.remove(&key);
            let _ = engine.end_session(old_sid).await;
        }

        if sid.is_none() {
            sid = Some(engine.create_session().await.map_err(|e| e.to_string())?);
            delta_prompt = full_prompt.clone();
        }

        Ok(PreparedSession {
            key: Some(key),
            sid,
            delta_prompt,
            full_prompt,
        })
    }

    fn commit_session_success(
        &mut self,
        key: Option<String>,
        sid: Option<SessionId>,
        full_prompt: &str,
        generated: &str,
    ) {
        let (Some(key), Some(sid)) = (key, sid) else {
            return;
        };

        let mut cached_prompt = String::with_capacity(full_prompt.len() + generated.len());
        cached_prompt.push_str(full_prompt);
        cached_prompt.push_str(generated);
        self.sessions
            .insert(key, SessionBinding { sid, cached_prompt });
    }

    #[on_runtime_control(GlobalUnload)]
    #[on_runtime_control(GlobalLoad)]
    async fn apply_runtime_control(&mut self, signal: RuntimeControlSignal) {
        match signal {
            RuntimeControlSignal::GlobalUnload { op_id } => {
                tracing::debug!(op_id, "llama runtime global unload");
                self.cleanup_runtime_state();
            }
            RuntimeControlSignal::GlobalLoad { op_id, payload } => {
                let _ = payload;
                tracing::debug!(op_id, "llama runtime global load pre-cleanup");
                // Runtime-level GlobalLoad is treated as a pre-load cleanup signal.
                // The actual model.load request is still driven by the management path.
                self.cleanup_runtime_state();
            }
        }
    }

    #[on_control_lagged]
    async fn on_control_lagged_cleanup(&mut self) {
        self.cleanup_runtime_state();
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

        let config: LlamaModelLoadConfig = match input.to_json() {
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
            let mut ctx_params = LlamaContextParams::default();
            if config.context_length > 0 {
                let context_length = config.context_length;
                ctx_params.n_ctx = context_length;
                if ctx_params.n_batch > context_length {
                    ctx_params.n_batch = context_length;
                }
                if ctx_params.n_ubatch > context_length {
                    ctx_params.n_ubatch = context_length;
                }
            }

            engine.load_model_with_workers(
                &config.model_path,
                LlamaModelParams::default(),
                ctx_params,
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
        let engine = match self.engine.as_ref() {
            Some(e) => Arc::clone(e),
            None => {
                let _ = reply_tx.send(BackendReply::Error(
                    "library not loaded; call lib.load first".into(),
                ));
                return;
            }
        };

        match engine.unload() {
            Ok(()) => {
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(
                    Arc::from([] as [u8; 0]),
                )));
            }
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
            }
        }

        self.sessions.clear();
    }

    fn apply_chat_template_if_possible(engine: &GGMLLlamaEngine, prompt: &str) -> String {
        let Some(parsed) = parse_role_prefixed_chat_prompt(prompt) else {
            return prompt.to_owned();
        };

        match engine.apply_chat_template(&parsed.messages, parsed.add_assistant_prompt) {
            Ok(applied) => applied,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "failed to apply llama chat template; falling back to raw prompt"
                );
                prompt.to_owned()
            }
        }
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
        let prompt = Self::apply_chat_template_if_possible(engine.as_ref(), &prompt);
        let prepared = match self
            .prepare_session(engine.as_ref(), session_key.as_deref(), prompt)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e));
                return;
            }
        };

        match engine
            .inference(&prepared.delta_prompt, max_tokens, prepared.sid)
            .await
        {
            Ok(text) => {
                self.commit_session_success(
                    prepared.key,
                    prepared.sid,
                    &prepared.full_prompt,
                    &text,
                );
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from(
                    text.as_bytes(),
                ))));
            }
            Err(e) => {
                if let (Some(key), Some(sid)) = (prepared.key, prepared.sid) {
                    self.sessions.remove(&key);
                    let _ = engine.end_session(sid).await;
                }
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
        let prompt = Self::apply_chat_template_if_possible(engine.as_ref(), prompt.as_ref());
        let prepared = match self
            .prepare_session(engine.as_ref(), session_key.as_deref(), prompt)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e));
                return;
            }
        };

        let (proto_tx, proto_rx) = mpsc::channel::<StreamChunk>(64);
        let _ = reply_tx.send(BackendReply::Stream(proto_rx));

        let (update_tx, update_rx) = tokio::sync::oneshot::channel::<Option<SessionUpdate>>();
        let engine_for_spawn = Arc::clone(&engine);

        tokio::spawn(async move {
            use crate::engine::ggml::llama::StreamChunk as LlamaChunk;
            let PreparedSession {
                key,
                sid,
                delta_prompt,
                full_prompt,
            } = prepared;

            match engine_for_spawn
                .inference_stream(&delta_prompt, max_tokens, sid)
                .await
            {
                Ok((mut llama_rx, new_sid)) => {
                    let mut generated = String::new();
                    let mut completed = false;
                    let mut forward_failed = false;
                    let mut stream_error = false;
                    while let Some(chunk) = llama_rx.recv().await {
                        let mapped = match chunk {
                            LlamaChunk::Token(t) => {
                                generated.push_str(&t);
                                StreamChunk::Token(t)
                            }
                            LlamaChunk::Done => {
                                completed = true;
                                StreamChunk::Done
                            }
                            LlamaChunk::Error(e) => {
                                stream_error = true;
                                StreamChunk::Error(e)
                            }
                        };

                        if proto_tx.send(mapped).await.is_err() {
                            forward_failed = true;
                            break;
                        }
                        if completed || stream_error {
                            break;
                        }
                    }
                    let update = if let Some(key) = key {
                        if completed && !forward_failed && !stream_error {
                            let mut cached_prompt =
                                String::with_capacity(full_prompt.len() + generated.len());
                            cached_prompt.push_str(&full_prompt);
                            cached_prompt.push_str(&generated);
                            Some(SessionUpdate::Keep {
                                key,
                                sid: new_sid,
                                cached_prompt,
                            })
                        } else {
                            Some(SessionUpdate::Drop { key, sid: new_sid })
                        }
                    } else {
                        let _ = engine_for_spawn.end_session(new_sid).await;
                        None
                    };
                    let _ = update_tx.send(update);
                }
                Err(e) => {
                    let _ = proto_tx.send(StreamChunk::Error(e.to_string())).await;
                    let update = match (key, sid) {
                        (Some(key), Some(sid)) => Some(SessionUpdate::Drop { key, sid }),
                        _ => None,
                    };
                    let _ = update_tx.send(update);
                }
            }
        });

        if let Ok(Some(update)) = update_rx.await {
            match update {
                SessionUpdate::Keep {
                    key,
                    sid,
                    cached_prompt,
                } => {
                    self.sessions
                        .insert(key, SessionBinding { sid, cached_prompt });
                }
                SessionUpdate::Drop { key, sid } => {
                    self.sessions.remove(&key);
                    let _ = engine.end_session(sid).await;
                }
            }
        }
    }
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Spawn a llama backend worker with a pre-loaded engine handle.
///
/// Used by `api::init` to separate library loading (phase 1) from worker
/// spawning (phase 2) so that no tasks are started if any library fails.
pub(crate) fn spawn_backend_with_engine(
    shared_ingress_rx: SharedIngressRx,
    control_tx: broadcast::Sender<WorkerCommand>,
    engine: Option<Arc<GGMLLlamaEngine>>,
) {
    let worker = LlamaWorker::new(engine);
    spawn_runtime_worker(shared_ingress_rx, control_tx.subscribe(), 0, worker);
}

#[cfg(test)]
mod tests {
    use super::{LlamaWorker, SessionBinding};
    use crate::scheduler::backend::protocol::RuntimeControlSignal;
    use crate::scheduler::types::Payload;

    #[tokio::test]
    async fn runtime_global_unload_clears_sessions() {
        let mut worker = LlamaWorker::new(None);
        worker.sessions.insert(
            "s1".to_owned(),
            SessionBinding {
                sid: 7,
                cached_prompt: String::new(),
            },
        );
        assert_eq!(worker.sessions.len(), 1);

        worker
            .apply_runtime_control(RuntimeControlSignal::GlobalUnload { op_id: 1 })
            .await;

        assert!(
            worker.sessions.is_empty(),
            "global unload should clear llama session mappings"
        );
    }

    #[tokio::test]
    async fn runtime_global_load_clears_sessions_before_load_attempt() {
        let mut worker = LlamaWorker::new(None);
        worker.sessions.insert(
            "s1".to_owned(),
            SessionBinding {
                sid: 9,
                cached_prompt: String::new(),
            },
        );
        assert_eq!(worker.sessions.len(), 1);

        worker
            .apply_runtime_control(RuntimeControlSignal::GlobalLoad {
                op_id: 2,
                payload: Payload::Json(serde_json::json!({
                    "model_path": "/tmp/model.gguf",
                    "num_workers": 1
                })),
            })
            .await;

        assert!(
            worker.sessions.is_empty(),
            "global load should clear stale llama session mappings before model load"
        );
    }
}
