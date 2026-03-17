//! Candle-based LLaMA engine adapter.
//!
//! Wraps [`candle_transformers`] quantised LLaMA inference so that the backend
//! worker can call a stable, engine-agnostic API.  The adapter supports GGUF
//! model files (the same format used by the GGML backend) via
//! [`candle_transformers::models::quantized_llama`].
//!
//! When the `candle` crate feature is disabled this module provides a stub
//! implementation that returns [`CandleLlamaEngineError::ModelNotLoaded`] for
//! every inference call, allowing the rest of the crate to compile without the
//! heavy Candle transitive dependency graph.

#[cfg(feature = "candle")]
use candle_core::{Device, Tensor};
#[cfg(feature = "candle")]
use candle_transformers::generation::{LogitsProcessor, Sampling};
#[cfg(feature = "candle")]
use candle_transformers::models::quantized_llama::ModelWeights;

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use super::errors::{CandleLlamaEngineError, SessionId, StreamChunk, StreamHandle};
use crate::engine::EngineError;

// ── Session state ─────────────────────────────────────────────────────────────

/// Internal state for one active KV-cache session.
#[derive(Debug, Clone)]
pub(crate) struct SessionState {
    /// Accumulated token history for this session (used for incremental input).
    pub tokens: Vec<u32>,
}

// ── Engine ────────────────────────────────────────────────────────────────────

/// Engine adapter wrapping a quantised LLaMA model loaded via `candle`.
///
/// Uses an [`Arc`] + [`RwLock`] over the inner state so that the engine handle
/// can be cheaply cloned into spawned tasks while serialising mutable access.
#[derive(Debug, Clone)]
pub struct CandleLlamaEngine {
    inner: Arc<RwLock<InnerState>>,
}

#[derive(Debug)]
struct InnerState {
    /// Loaded model weights; `None` when no model has been loaded.
    #[cfg(feature = "candle")]
    model: Option<ModelWeights>,
    #[cfg(not(feature = "candle"))]
    model: Option<()>,
    /// Loaded tokenizer; `None` when no model has been loaded.
    #[cfg(feature = "candle")]
    tokenizer: Option<tokenizers::Tokenizer>,
    #[cfg(not(feature = "candle"))]
    tokenizer: Option<()>,
    /// Random seed used for sampling.
    seed: u64,
    /// Per-session KV cache state.
    sessions: std::collections::HashMap<SessionId, SessionState>,
    /// Session ID counter.
    next_session_id: SessionId,
}

impl CandleLlamaEngine {
    /// Create a new, empty engine (no model loaded).
    pub fn new(seed: u64) -> Self {
        Self {
            inner: Arc::new(RwLock::new(InnerState {
                model: None,
                tokenizer: None,
                seed,
                sessions: std::collections::HashMap::new(),
                next_session_id: 1,
            })),
        }
    }

    /// Resolve the tokenizer path: use explicit path, or look for
    /// `tokenizer.json` in the same directory as the model file.
    fn resolve_tokenizer(
        model_path: &Path,
        tokenizer_path: Option<&str>,
    ) -> Result<PathBuf, EngineError> {
        if let Some(p) = tokenizer_path {
            return Ok(PathBuf::from(p));
        }
        let dir = model_path.parent().unwrap_or(Path::new("."));
        let candidate = dir.join("tokenizer.json");
        if candidate.exists() {
            Ok(candidate)
        } else {
            Err(CandleLlamaEngineError::TokenizerNotFound {
                dir: dir.display().to_string(),
            }
            .into())
        }
    }

    /// Load model weights from `model_path` (GGUF format).
    ///
    /// `tokenizer_path`, when `None`, falls back to `<model_dir>/tokenizer.json`.
    pub fn load_model(
        &self,
        model_path: &str,
        tokenizer_path: Option<&str>,
        seed: u64,
    ) -> Result<(), EngineError> {
        let path = Path::new(model_path);
        let tok_path = Self::resolve_tokenizer(path, tokenizer_path)?;

        #[cfg(feature = "candle")]
        {
            use candle_core::quantized::gguf_file;
            use std::fs::File;

            tracing::info!(model_path, "loading candle llama model (GGUF)");

            let mut model_file = File::open(path).map_err(|e| CandleLlamaEngineError::LoadModel {
                model_path: model_path.to_owned(),
                message: e.to_string(),
            })?;

            let gguf =
                gguf_file::Content::read(&mut model_file).map_err(|e| {
                    CandleLlamaEngineError::LoadModel {
                        model_path: model_path.to_owned(),
                        message: e.to_string(),
                    }
                })?;

            let device = Device::Cpu;
            let weights =
                ModelWeights::from_gguf(gguf, &mut model_file, &device).map_err(|e| {
                    CandleLlamaEngineError::LoadModel {
                        model_path: model_path.to_owned(),
                        message: e.to_string(),
                    }
                })?;

            tracing::info!(
                tokenizer_path = %tok_path.display(),
                "loading candle llama tokenizer"
            );
            let tokenizer = tokenizers::Tokenizer::from_file(&tok_path).map_err(|e| {
                CandleLlamaEngineError::LoadTokenizer {
                    tokenizer_path: tok_path.display().to_string(),
                    message: e.to_string(),
                }
            })?;

            let mut state = self.inner.write().map_err(|_| {
                CandleLlamaEngineError::LockPoisoned {
                    operation: "write model state",
                }
            })?;
            state.model = Some(weights);
            state.tokenizer = Some(tokenizer);
            state.seed = seed;
            state.sessions.clear();
            state.next_session_id = 1;
        }

        #[cfg(not(feature = "candle"))]
        {
            let _ = (tok_path, seed);
            tracing::warn!(
                "candle feature is not enabled; model.load is a no-op for CandleLlamaEngine"
            );
        }

        Ok(())
    }

    /// Unload the current model and clear all sessions.
    pub fn unload(&self) -> Result<(), EngineError> {
        let mut state = self.inner.write().map_err(|_| CandleLlamaEngineError::LockPoisoned {
            operation: "write model state for unload",
        })?;
        state.model = None;
        state.tokenizer = None;
        state.sessions.clear();
        Ok(())
    }

    /// Returns `true` when a model is currently loaded.
    pub fn is_model_loaded(&self) -> bool {
        self.inner
            .read()
            .map(|s| s.model.is_some())
            .unwrap_or(false)
    }

    /// Create a new session and return its ID.
    pub async fn create_session(&self) -> Result<SessionId, EngineError> {
        let mut state = self.inner.write().map_err(|_| CandleLlamaEngineError::LockPoisoned {
            operation: "create session",
        })?;
        let sid = state.next_session_id;
        state.next_session_id += 1;
        state.sessions.insert(sid, SessionState { tokens: Vec::new() });
        Ok(sid)
    }

    /// End a session and release its KV cache.
    pub async fn end_session(&self, session_id: SessionId) -> Result<(), EngineError> {
        let mut state = self.inner.write().map_err(|_| CandleLlamaEngineError::LockPoisoned {
            operation: "end session",
        })?;
        state.sessions.remove(&session_id);
        Ok(())
    }

    /// Run synchronous (non-streaming) text generation.
    ///
    /// Returns the generated text as a `String`.
    pub async fn inference(
        &self,
        prompt: &str,
        max_tokens: usize,
        session_id: Option<SessionId>,
    ) -> Result<String, EngineError> {
        #[cfg(feature = "candle")]
        {
            self.run_inference_blocking(prompt, max_tokens, session_id)
                .await
        }
        #[cfg(not(feature = "candle"))]
        {
            let _ = (prompt, max_tokens, session_id);
            Err(CandleLlamaEngineError::ModelNotLoaded.into())
        }
    }

    /// Run streaming text generation.
    ///
    /// Returns a [`StreamHandle`] that yields [`StreamChunk`] tokens.  The
    /// caller **must** drive the stream to completion and end the session.
    pub async fn inference_stream(
        &self,
        prompt: &str,
        max_tokens: usize,
        session_id: Option<SessionId>,
    ) -> Result<(StreamHandle, SessionId), EngineError> {
        #[cfg(feature = "candle")]
        {
            let (tx, rx) = tokio::sync::mpsc::channel::<StreamChunk>(64);
            let sid = match session_id {
                Some(s) => s,
                None => self.create_session().await?,
            };
            let engine = self.clone();
            let prompt = prompt.to_owned();
            tokio::task::spawn_blocking(move || {
                engine.run_stream_blocking(&prompt, max_tokens, sid, tx);
            });
            Ok((rx, sid))
        }
        #[cfg(not(feature = "candle"))]
        {
            let _ = (prompt, max_tokens, session_id);
            Err(CandleLlamaEngineError::ModelNotLoaded.into())
        }
    }

    // ── Internal blocking helpers (candle feature only) ───────────────────────

    #[cfg(feature = "candle")]
    async fn run_inference_blocking(
        &self,
        prompt: &str,
        max_tokens: usize,
        session_id: Option<SessionId>,
    ) -> Result<String, EngineError> {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamChunk>(64);
        let sid = match session_id {
            Some(s) => s,
            None => self.create_session().await?,
        };
        let engine = self.clone();
        let prompt = prompt.to_owned();
        tokio::task::spawn_blocking(move || {
            engine.run_stream_blocking(&prompt, max_tokens, sid, tx);
        });

        let mut output = String::new();
        while let Some(chunk) = rx.recv().await {
            match chunk {
                StreamChunk::Token(t) => output.push_str(&t),
                StreamChunk::Done => break,
                StreamChunk::Error(e) => {
                    return Err(CandleLlamaEngineError::InferenceStreamError { message: e }.into())
                }
            }
        }
        Ok(output)
    }

    #[cfg(feature = "candle")]
    fn run_stream_blocking(
        &self,
        prompt: &str,
        max_tokens: usize,
        session_id: SessionId,
        tx: tokio::sync::mpsc::Sender<StreamChunk>,
    ) {
        use candle_core::{Device, IndexOp};
        use candle_transformers::generation::Sampling;
        use std::io::Write;

        let send = |chunk: StreamChunk| tx.blocking_send(chunk).is_ok();

        // Acquire read lock to borrow model + tokenizer.
        let state = match self.inner.read() {
            Ok(s) => s,
            Err(_) => {
                send(StreamChunk::Error("lock poisoned".into()));
                return;
            }
        };

        let (model_ref, tokenizer_ref) = match (&state.model, &state.tokenizer) {
            (Some(m), Some(t)) => (m, t),
            _ => {
                send(StreamChunk::Error("model not loaded".into()));
                return;
            }
        };

        // Tokenize prompt.
        let encoding = match tokenizer_ref.encode(prompt, true) {
            Ok(e) => e,
            Err(e) => {
                send(StreamChunk::Error(format!("tokenize failed: {e}")));
                return;
            }
        };
        let mut tokens: Vec<u32> = encoding.get_ids().to_vec();

        // Retrieve any prefix tokens already in the session's KV cache.
        drop(state); // release read lock before acquiring write lock
        {
            if let Ok(mut state) = self.inner.write() {
                if let Some(sess) = state.sessions.get(&session_id) {
                    // Only append the new tokens (delta after cached prefix).
                    let cached = &sess.tokens;
                    if tokens.starts_with(cached) && tokens.len() > cached.len() {
                        tokens = tokens[cached.len()..].to_vec();
                    }
                }
            }
        }

        // Re-acquire read lock for inference.
        let state = match self.inner.read() {
            Ok(s) => s,
            Err(_) => {
                send(StreamChunk::Error("lock poisoned during inference".into()));
                return;
            }
        };
        let (model, tokenizer) = match (&state.model, &state.tokenizer) {
            (Some(m), Some(t)) => (m, t),
            _ => {
                send(StreamChunk::Error("model not loaded".into()));
                return;
            }
        };

        let device = Device::Cpu;
        let mut all_tokens = tokens.clone();
        let mut logits_processor = {
            LogitsProcessor::from_sampling(
                state.seed,
                Sampling::ArgMax,
            )
        };

        // Run forward pass token-by-token.
        // NOTE: ModelWeights::forward takes &mut self, so we need write access.
        // Drop read lock and use write lock for the forward pass.
        drop(state);

        let max_new_tokens = max_tokens.min(4096);
        for _ in 0..max_new_tokens {
            let input = match Tensor::new(all_tokens.as_slice(), &device)
                .and_then(|t| t.unsqueeze(0))
            {
                Ok(t) => t,
                Err(e) => {
                    send(StreamChunk::Error(format!("tensor error: {e}")));
                    return;
                }
            };

            let logits = {
                let mut state = match self.inner.write() {
                    Ok(s) => s,
                    Err(_) => {
                        send(StreamChunk::Error("lock poisoned".into()));
                        return;
                    }
                };
                let model = match state.model.as_mut() {
                    Some(m) => m,
                    None => {
                        send(StreamChunk::Error("model not loaded".into()));
                        return;
                    }
                };
                match model.forward(&input, all_tokens.len() - 1) {
                    Ok(l) => l,
                    Err(e) => {
                        send(StreamChunk::Error(format!("forward pass error: {e}")));
                        return;
                    }
                }
            };

            let logits = match logits.squeeze(0) {
                Ok(l) => l,
                Err(e) => {
                    send(StreamChunk::Error(format!("squeeze error: {e}")));
                    return;
                }
            };

            let next_token = match logits_processor.sample(&logits) {
                Ok(t) => t,
                Err(e) => {
                    send(StreamChunk::Error(format!("sampling error: {e}")));
                    return;
                }
            };

            // EOS check using the tokenizer's special token.
            let state = match self.inner.read() {
                Ok(s) => s,
                Err(_) => {
                    send(StreamChunk::Error("lock poisoned".into()));
                    return;
                }
            };
            let tokenizer = match state.tokenizer.as_ref() {
                Some(t) => t,
                None => {
                    send(StreamChunk::Error("tokenizer not loaded".into()));
                    return;
                }
            };
            let eos_token = tokenizer
                .token_to_id("</s>")
                .or_else(|| tokenizer.token_to_id("<|end_of_text|>"))
                .or_else(|| tokenizer.token_to_id("<|endoftext|>"))
                .unwrap_or(2);
            drop(state);

            if next_token == eos_token {
                break;
            }

            all_tokens.push(next_token);

            // Decode the new token to text.
            let state = match self.inner.read() {
                Ok(s) => s,
                Err(_) => {
                    send(StreamChunk::Error("lock poisoned".into()));
                    return;
                }
            };
            let token_text = state
                .tokenizer
                .as_ref()
                .and_then(|t| t.id_to_token(next_token).map(|s| s.replace('▁', " ")))
                .unwrap_or_default();
            drop(state);

            if !send(StreamChunk::Token(token_text)) {
                break;
            }
        }

        // Update session token cache.
        if let Ok(mut state) = self.inner.write() {
            if let Some(sess) = state.sessions.get_mut(&session_id) {
                sess.tokens = all_tokens;
            }
        }

        send(StreamChunk::Done);
    }
}
