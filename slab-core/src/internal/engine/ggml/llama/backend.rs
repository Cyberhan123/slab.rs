//! Backend worker adapter for `ggml.llama`.
//!
//! Provides [`spawn_backend`] and [`spawn_backend_with_path`] which start a
//! Tokio task translating [`BackendRequest`] messages into llama inference calls.
//!
//! # Supported ops
//!
//! | Op string            | Event variant    | Description                                    |
//! |----------------------|------------------|------------------------------------------------|
//! | `"model.load"`       | `LoadModel`      | Load a GGUF model from the engine.             |
//! | `"model.unload"`     | `UnloadModel`    | Drop the model handle; call model.load to restore. |
//! | `"inference"`        | `Inference`      | Unary text generation; input is UTF-8 prompt.  |
//! | `"inference.stream"` | `InferenceStream`| Streaming text generation.                     |
//!
//! ### `model.load` input JSON
//! ```json
//! { "model_path": "/path/to/model.gguf", "num_workers": 1, "context_length": 4096 }
//! ```
//!
//! ## Grammar constraints (optional)
//!
//! Callers may request GBNF grammar-constrained sampling via options keys:
//!
//! | Key               | Type    | Description                                              |
//! |-------------------|---------|----------------------------------------------------------|
//! | `grammar`         | string  | Raw GBNF grammar string (highest priority).             |
//! | `grammar_json`    | bool    | Use built-in JSON grammar when `true`.                  |
//! | `grammar_tool_call` | bool  | Use built-in tool-call grammar when `true`.             |
//!
//! Priority: `grammar` > `grammar_json` > `grammar_tool_call`.
//!
//! If grammar initialization fails for any reason (invalid GBNF, null vocab,
//! unsupported runtime) a warning is logged and generation falls back to
//! unconstrained sampling — existing behavior is never broken.

use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;
use slab_llama::ChatMessage as LlamaChatMessage;
use slab_types::chat::ConversationMessage;
use tokio::sync::{broadcast, mpsc, watch};

use crate::internal::engine::ggml::llama::adapter::GGMLLlamaEngine;
use crate::internal::engine::ggml::llama::errors::SessionId;
use crate::internal::scheduler::backend::protocol::{
    BackendReply, BackendRequest, RuntimeControlSignal, StreamChunk, WorkerCommand,
};
use crate::internal::scheduler::backend::runner::{spawn_runtime_worker, SharedIngressRx};
use crate::internal::scheduler::types::Payload;
use slab_core_macros::backend_handler;

// ── Built-in grammar strings ──────────────────────────────────────────────────

/// Minimal GBNF grammar that constrains output to a valid JSON value.
///
/// Supports objects, arrays, strings, numbers, booleans and null.
/// Based on the grammar shipped with llama.cpp examples.
pub(crate) const GRAMMAR_JSON: &str = r#"root   ::= value
value  ::= object | array | string | number | "true" | "false" | "null"
object ::=
  "{" ws (
            string ":" ws value
    ("," ws string ":" ws value)*
  )? "}" ws
array  ::=
  "[" ws (
            value
    ("," ws value)*
  )? "]" ws
string ::=
  "\"" (
    [^\\"\x7F\x00-\x1F] |
    "\\" (["\\/bfnrt] | "u" [0-9a-fA-F] [0-9a-fA-F] [0-9a-fA-F] [0-9a-fA-F])
  )* "\"" ws
number ::= ("-"? ([0-9] | [1-9] [0-9]*)) ("." [0-9]+)? (([eE] [-+]? [0-9]+))? ws
ws     ::= ([ \t\n] ws)?
"#;

/// GBNF grammar for tool-call envelope: `{"tool":"<name>","arguments":{...}}`.
///
/// Constrains output to a JSON object that contains exactly the keys `"tool"`
/// (string) and `"arguments"` (JSON object), in that order.
pub(crate) const GRAMMAR_TOOL_CALL: &str = r#"root      ::= "{" ws "\"tool\"" ws ":" ws string ws "," ws "\"arguments\"" ws ":" ws object ws "}"
object    ::=
  "{" ws (
            string ":" ws value
    ("," ws string ":" ws value)*
  )? "}" ws
value     ::= object | array | string | number | "true" | "false" | "null"
array     ::=
  "[" ws (
            value
    ("," ws value)*
  )? "]" ws
string    ::=
  "\"" (
    [^\\"\x7F\x00-\x1F] |
    "\\" (["\\/bfnrt] | "u" [0-9a-fA-F] [0-9a-fA-F] [0-9a-fA-F] [0-9a-fA-F])
  )* "\"" ws
number    ::= ("-"? ([0-9] | [1-9] [0-9]*)) ("." [0-9]+)? (([eE] [-+]? [0-9]+))? ws
ws        ::= ([ \t\n] ws)?
"#;

/// Resolve the active grammar string from the options map.
///
/// Priority: `grammar` (raw GBNF) > `grammar_json` > `grammar_tool_call`.
/// Returns `None` when no grammar option is set or when the raw grammar string
/// is empty.
fn resolve_grammar(opts: &serde_json::Value) -> Option<String> {
    // Highest priority: raw GBNF string.
    if let Some(g) = opts.get("grammar").and_then(|v| v.as_str()) {
        if !g.is_empty() {
            return Some(g.to_owned());
        }
    }
    // Second: built-in JSON grammar.
    if opts.get("grammar_json").and_then(|v| v.as_bool()).unwrap_or(false) {
        return Some(GRAMMAR_JSON.to_owned());
    }
    // Third: built-in tool-call grammar.
    if opts.get("grammar_tool_call").and_then(|v| v.as_bool()).unwrap_or(false) {
        return Some(GRAMMAR_TOOL_CALL.to_owned());
    }
    None
}

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
        messages.push(LlamaChatMessage { role, content: raw_content.trim_start().to_owned() });
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

    Some(ParsedChatPrompt { messages, add_assistant_prompt })
}

// ── Worker ────────────────────────────────────────────────────────────────────

/// Deserialize a `chat_messages` JSON array from the options map into a
/// `Vec<LlamaChatMessage>`.  Returns an empty Vec when the key is absent or
/// the value is not an array.
fn extract_chat_messages(opts: &serde_json::Value) -> Vec<LlamaChatMessage> {
    let Some(raw) = opts.get("chat_messages") else {
        return Vec::new();
    };

    raw.as_array()
        .into_iter()
        .flatten()
        .filter_map(|value| serde_json::from_value::<ConversationMessage>(value.clone()).ok())
        .filter(|message| !message.role.trim().is_empty() && message.has_meaningful_content())
        .into_iter()
        .map(|message| LlamaChatMessage {
            role: normalize_chat_role_for_template(&message.role).to_owned(),
            content: message.rendered_text(),
        })
        .collect()
}

fn normalize_chat_role_for_template(role: &str) -> &'static str {
    match role {
        "system" | "developer" => "system",
        "assistant" => "assistant",
        _ => "user",
    }
}

/// Infer the `add_assistant_prompt` flag from the message list.
///
/// Returns `false` when the last message already has role `"assistant"` (i.e.
/// an assistant-prefill / regeneration scenario where the template should not
/// append another opening assistant turn).  Returns `true` in all other cases.
fn infer_add_assistant_prompt(messages: &[LlamaChatMessage]) -> bool {
    messages.last().map(|m| m.role != "assistant").unwrap_or(true)
}

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
    Keep { key: String, sid: SessionId, cached_prompt: String },
    Drop { key: String, sid: SessionId },
}

#[backend_handler]
impl LlamaWorker {
    fn new(engine: Option<Arc<GGMLLlamaEngine>>) -> Self {
        Self { engine, sessions: HashMap::new() }
    }

    #[on_event(LoadModel)]
    async fn on_load_model(&mut self, req: BackendRequest) {
        let BackendRequest { input, reply_tx, .. } = req;
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
        let BackendRequest { input, reply_tx, .. } = req;
        let opts = invocation.options.to_serde_value();
        let max_tokens =
            opts.get("max_tokens").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(256);
        let session_key = opts.get("session_key").and_then(|s| s.as_str()).map(str::to_owned);
        let apply_chat_template =
            opts.get("apply_chat_template").and_then(|v| v.as_bool()).unwrap_or(false);
        let chat_messages = extract_chat_messages(&opts);
        let grammar = resolve_grammar(&opts);
        self.handle_inference(
            input,
            max_tokens,
            session_key,
            apply_chat_template,
            chat_messages,
            grammar,
            reply_tx,
        )
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
        let BackendRequest { input, cancel_rx, reply_tx, .. } = req;
        let opts = invocation.options.to_serde_value();
        let max_tokens =
            opts.get("max_tokens").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(256);
        let session_key = opts.get("session_key").and_then(|s| s.as_str()).map(str::to_owned);
        let apply_chat_template =
            opts.get("apply_chat_template").and_then(|v| v.as_bool()).unwrap_or(false);
        let chat_messages = extract_chat_messages(&opts);
        let grammar = resolve_grammar(&opts);
        self.handle_inference_stream(
            input,
            max_tokens,
            session_key,
            apply_chat_template,
            chat_messages,
            grammar,
            cancel_rx,
            reply_tx,
        )
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
        grammar: Option<String>,
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
            // Create a new session with the grammar constraint applied to the
            // sampler chain.  If the same key is later reused with a different
            // grammar the session is reset (stale_sid path above) and a fresh
            // session with the new grammar is created here.
            sid =
                Some(engine.create_session_with_grammar(grammar).await.map_err(|e| e.to_string())?);
            delta_prompt = full_prompt.clone();
        }

        Ok(PreparedSession { key: Some(key), sid, delta_prompt, full_prompt })
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
        self.sessions.insert(key, SessionBinding { sid, cached_prompt });
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

    // ── model.load ────────────────────────────────────────────────────────────

    async fn handle_load_model(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
    ) {
        let engine = match self.engine.as_ref() {
            Some(e) => Arc::clone(e),
            None => {
                let _ = reply_tx.send(BackendReply::Error("engine not initialized".into()));
                return;
            }
        };

        let config: LlamaModelLoadConfig = match input.to_json() {
            Ok(c) => c,
            Err(e) => {
                let _ =
                    reply_tx.send(BackendReply::Error(format!("invalid model.load config: {e}")));
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
                let _ =
                    reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from([] as [u8; 0]))));
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
                let _ = reply_tx.send(BackendReply::Error("engine not initialized".into()));
                return;
            }
        };

        match engine.unload() {
            Ok(()) => {
                let _ =
                    reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from([] as [u8; 0]))));
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
        apply_chat_template: bool,
        chat_messages: Vec<LlamaChatMessage>,
        grammar: Option<String>,
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
        let prompt = if apply_chat_template && !chat_messages.is_empty() {
            let add_assistant = infer_add_assistant_prompt(&chat_messages);
            match engine.apply_chat_template(&chat_messages, add_assistant) {
                Ok(applied) => applied,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "failed to apply embedded chat template; falling back to raw prompt"
                    );
                    prompt
                }
            }
        } else {
            Self::apply_chat_template_if_possible(engine.as_ref(), &prompt)
        };
        let prepared = match self
            .prepare_session(engine.as_ref(), session_key.as_deref(), prompt, grammar.clone())
            .await
        {
            Ok(v) => v,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e));
                return;
            }
        };

        match engine.inference(&prepared.delta_prompt, max_tokens, prepared.sid, grammar).await {
            Ok(text) => {
                self.commit_session_success(
                    prepared.key,
                    prepared.sid,
                    &prepared.full_prompt,
                    &text,
                );
                let _ =
                    reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from(text.as_bytes()))));
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
        apply_chat_template: bool,
        chat_messages: Vec<LlamaChatMessage>,
        grammar: Option<String>,
        cancel_rx: watch::Receiver<bool>,
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
        let prompt = if apply_chat_template && !chat_messages.is_empty() {
            let add_assistant = infer_add_assistant_prompt(&chat_messages);
            match engine.apply_chat_template(&chat_messages, add_assistant) {
                Ok(applied) => applied,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "failed to apply embedded chat template; falling back to raw prompt"
                    );
                    prompt.as_ref().to_owned()
                }
            }
        } else {
            Self::apply_chat_template_if_possible(engine.as_ref(), prompt.as_ref())
        };
        let prepared = match self
            .prepare_session(engine.as_ref(), session_key.as_deref(), prompt, grammar.clone())
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
        let mut cancel_rx = cancel_rx;

        tokio::spawn(async move {
            use crate::internal::engine::ggml::llama::StreamChunk as LlamaChunk;
            let PreparedSession { key, sid, delta_prompt, full_prompt } = prepared;

            match engine_for_spawn.inference_stream(&delta_prompt, max_tokens, sid, grammar).await {
                Ok((mut llama_rx, new_sid)) => {
                    let mut generated = String::new();
                    let mut completed = false;
                    let mut forward_failed = false;
                    let mut stream_error = false;
                    let mut cancelled = false;
                    loop {
                        tokio::select! {
                            cancel_changed = cancel_rx.changed(), if !completed && !stream_error && !forward_failed => {
                                let cancel_requested = if cancel_changed.is_ok() {
                                    *cancel_rx.borrow()
                                } else {
                                    false
                                };
                                if cancel_requested {
                                    cancelled = true;
                                    if let Err(error) = engine_for_spawn.cancel_generate(new_sid).await {
                                        tracing::warn!(
                                            session_id = new_sid,
                                            error = %error,
                                            "failed to cancel llama generation"
                                        );
                                    }
                                } else if cancel_changed.is_ok() {
                                    continue;
                                }
                                break;
                            }
                            chunk = llama_rx.recv() => {
                                let Some(chunk) = chunk else {
                                    break;
                                };

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
                                    if !completed && !stream_error {
                                        if let Err(error) = engine_for_spawn.cancel_generate(new_sid).await {
                                            tracing::warn!(
                                                session_id = new_sid,
                                                error = %error,
                                                "failed to cancel llama generation after downstream disconnect"
                                            );
                                        }
                                    }
                                    break;
                                }
                                if completed || stream_error {
                                    break;
                                }
                            }
                        }
                    }
                    let update = if let Some(key) = key {
                        if completed && !forward_failed && !stream_error && !cancelled {
                            let mut cached_prompt =
                                String::with_capacity(full_prompt.len() + generated.len());
                            cached_prompt.push_str(&full_prompt);
                            cached_prompt.push_str(&generated);
                            Some(SessionUpdate::Keep { key, sid: new_sid, cached_prompt })
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
                SessionUpdate::Keep { key, sid, cached_prompt } => {
                    self.sessions.insert(key, SessionBinding { sid, cached_prompt });
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
/// Used by runtime construction to separate library loading (phase 1) from
/// worker spawning (phase 2) so that no tasks are started if any library fails.
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
    use super::{
        extract_chat_messages, infer_add_assistant_prompt, resolve_grammar, LlamaWorker,
        SessionBinding,
    };
    use crate::internal::scheduler::backend::protocol::RuntimeControlSignal;
    use crate::internal::scheduler::types::Payload;

    // ── infer_add_assistant_prompt ────────────────────────────────────────────

    fn msg(role: &str, content: &str) -> super::LlamaChatMessage {
        super::LlamaChatMessage { role: role.to_owned(), content: content.to_owned() }
    }

    #[test]
    fn infer_add_assistant_returns_true_for_empty_messages() {
        assert!(infer_add_assistant_prompt(&[]), "empty list should default to add_assistant=true");
    }

    #[test]
    fn infer_add_assistant_returns_true_when_last_role_is_user() {
        let messages = vec![msg("user", "hello")];
        assert!(
            infer_add_assistant_prompt(&messages),
            "user-last messages should yield add_assistant=true"
        );
    }

    #[test]
    fn infer_add_assistant_returns_false_when_last_role_is_assistant() {
        let messages = vec![msg("user", "hello"), msg("assistant", "hi there")];
        assert!(
            !infer_add_assistant_prompt(&messages),
            "assistant-last messages (prefill) should yield add_assistant=false"
        );
    }

    // ── extract_chat_messages ─────────────────────────────────────────────────

    #[test]
    fn extract_chat_messages_returns_empty_when_key_absent() {
        let opts = serde_json::json!({ "other_key": "value" });
        let result = extract_chat_messages(&opts);
        assert!(result.is_empty(), "missing key should yield empty vec");
    }

    #[test]
    fn extract_chat_messages_returns_empty_when_value_is_not_array() {
        let opts = serde_json::json!({ "chat_messages": "not an array" });
        let result = extract_chat_messages(&opts);
        assert!(result.is_empty(), "non-array value should yield empty vec");
    }

    #[test]
    fn extract_chat_messages_skips_malformed_entries() {
        let opts = serde_json::json!({
            "chat_messages": [
                { "role": "user", "content": "hello" },
                { "role": "user" },             // missing content → skipped
                { "content": "no role" },       // missing role → skipped
                42,                             // wrong type → skipped
            ]
        });
        let result = extract_chat_messages(&opts);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "user");
        assert_eq!(result[0].content, "hello");
    }

    #[test]
    fn extract_chat_messages_round_trips_valid_array() {
        let opts = serde_json::json!({
            "chat_messages": [
                { "role": "system", "content": "You are a helpful assistant." },
                { "role": "user", "content": "Hi!" },
                { "role": "assistant", "content": "Hello there!" },
            ]
        });
        let result = extract_chat_messages(&opts);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].role, "system");
        assert_eq!(result[0].content, "You are a helpful assistant.");
        assert_eq!(result[1].role, "user");
        assert_eq!(result[1].content, "Hi!");
        assert_eq!(result[2].role, "assistant");
        assert_eq!(result[2].content, "Hello there!");
    }

    // ── LlamaWorker session management ────────────────────────────────────────

    #[tokio::test]
    async fn runtime_global_unload_clears_sessions() {
        let mut worker = LlamaWorker::new(None);
        worker
            .sessions
            .insert("s1".to_owned(), SessionBinding { sid: 7, cached_prompt: String::new() });
        assert_eq!(worker.sessions.len(), 1);

        worker.apply_runtime_control(RuntimeControlSignal::GlobalUnload { op_id: 1 }).await;

        assert!(worker.sessions.is_empty(), "global unload should clear llama session mappings");
    }

    #[tokio::test]
    async fn runtime_global_load_clears_sessions_before_load_attempt() {
        let mut worker = LlamaWorker::new(None);
        worker
            .sessions
            .insert("s1".to_owned(), SessionBinding { sid: 9, cached_prompt: String::new() });
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

    // ── resolve_grammar ───────────────────────────────────────────────────────

    #[test]
    fn resolve_grammar_returns_none_when_no_grammar_options_set() {
        let opts = serde_json::json!({ "max_tokens": 64 });
        assert!(resolve_grammar(&opts).is_none(), "no grammar options should yield None");
    }

    #[test]
    fn resolve_grammar_returns_raw_grammar_string() {
        let gbnf = "root ::= \"hello\"";
        let opts = serde_json::json!({ "grammar": gbnf });
        assert_eq!(resolve_grammar(&opts).as_deref(), Some(gbnf));
    }

    #[test]
    fn resolve_grammar_ignores_empty_grammar_string() {
        let opts = serde_json::json!({ "grammar": "" });
        assert!(resolve_grammar(&opts).is_none(), "empty grammar string should yield None");
    }

    #[test]
    fn resolve_grammar_returns_json_grammar_when_flag_true() {
        let opts = serde_json::json!({ "grammar_json": true });
        let result = resolve_grammar(&opts);
        assert!(result.is_some(), "grammar_json=true should yield Some");
        assert!(result.unwrap().contains("root"), "JSON grammar should contain a root rule");
    }

    #[test]
    fn resolve_grammar_returns_tool_call_grammar_when_flag_true() {
        let opts = serde_json::json!({ "grammar_tool_call": true });
        let result = resolve_grammar(&opts);
        assert!(result.is_some(), "grammar_tool_call=true should yield Some");
        assert!(
            result.unwrap().contains("tool"),
            "tool-call grammar should reference the tool key"
        );
    }

    #[test]
    fn resolve_grammar_raw_takes_precedence_over_json_flag() {
        let gbnf = "root ::= \"raw\"";
        let opts = serde_json::json!({ "grammar": gbnf, "grammar_json": true });
        assert_eq!(
            resolve_grammar(&opts).as_deref(),
            Some(gbnf),
            "raw grammar should win over grammar_json flag"
        );
    }

    #[test]
    fn resolve_grammar_json_flag_takes_precedence_over_tool_call_flag() {
        let opts = serde_json::json!({ "grammar_json": true, "grammar_tool_call": true });
        let result = resolve_grammar(&opts).expect("should yield Some");
        // JSON grammar contains `value` rule; tool-call grammar contains `arguments` literal.
        assert!(
            !result.contains("arguments"),
            "grammar_json should take precedence over grammar_tool_call"
        );
    }
}
