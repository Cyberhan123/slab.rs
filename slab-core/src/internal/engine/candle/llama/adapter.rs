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
use candle_core::{Device, Tensor as CandleTensor};
#[cfg(feature = "candle")]
use candle_transformers::generation::LogitsProcessor;
#[cfg(feature = "candle")]
use candle_transformers::models::quantized_llama::ModelWeights;

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use super::errors::{CandleLlamaEngineError, SessionId, StreamChunk, StreamHandle};
use crate::internal::engine::EngineError;

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
        tokenizer_path: Option<&Path>,
    ) -> Result<PathBuf, EngineError> {
        if let Some(p) = tokenizer_path {
            return Ok(p.to_path_buf());
        }
        let dir = model_path.parent().unwrap_or(Path::new("."));
        let candidate = dir.join("tokenizer.json");
        if candidate.exists() {
            Ok(candidate)
        } else {
            Err(CandleLlamaEngineError::TokenizerNotFound { dir: dir.display().to_string() }.into())
        }
    }

    /// Load model weights from `model_path` (GGUF format).
    ///
    /// `tokenizer_path`, when `None`, falls back to `<model_dir>/tokenizer.json`.
    pub fn load_model(
        &self,
        model_path: &Path,
        tokenizer_path: Option<&Path>,
        seed: u64,
    ) -> Result<(), EngineError> {
        let tok_path = Self::resolve_tokenizer(model_path, tokenizer_path)?;

        #[cfg(feature = "candle")]
        {
            use candle_core::quantized::gguf_file;
            use std::fs::File;

            tracing::info!(model_path = %model_path.display(), "loading candle llama model (GGUF)");

            let mut model_file =
                File::open(model_path).map_err(|e| CandleLlamaEngineError::LoadModel {
                    model_path: model_path.display().to_string(),
                    message: e.to_string(),
                })?;

            let gguf = gguf_file::Content::read(&mut model_file).map_err(|e| {
                CandleLlamaEngineError::LoadModel {
                    model_path: model_path.display().to_string(),
                    message: e.to_string(),
                }
            })?;

            let device = Device::Cpu;
            let weights = ModelWeights::from_gguf(gguf, &mut model_file, &device).map_err(|e| {
                CandleLlamaEngineError::LoadModel {
                    model_path: model_path.display().to_string(),
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
                CandleLlamaEngineError::LockPoisoned { operation: "write model state" }
            })?;
            state.model = Some(weights);
            state.tokenizer = Some(tokenizer);
            state.seed = seed;
            state.sessions.clear();
            state.next_session_id = 1;
        }

        #[cfg(not(feature = "candle"))]
        {
            let _ = (model_path, tok_path, seed);
            tracing::warn!(
                "candle feature is not enabled; model.load is a no-op for CandleLlamaEngine"
            );
        }

        Ok(())
    }

    /// Shared unload logic used by both the inherent method and the
    /// [`ModelLoader`] trait implementation.
    fn do_unload(&self) -> Result<(), CandleLlamaEngineError> {
        let mut state = self.inner.write().map_err(|_| CandleLlamaEngineError::LockPoisoned {
            operation: "write model state for unload",
        })?;
        state.model = None;
        state.tokenizer = None;
        state.sessions.clear();
        Ok(())
    }

    /// Unload the current model and clear all sessions.
    pub fn unload(&self) -> Result<(), EngineError> {
        Ok(self.do_unload()?)
    }

    /// Create a new session and return its ID.
    pub async fn create_session(&self) -> Result<SessionId, EngineError> {
        let mut state = self
            .inner
            .write()
            .map_err(|_| CandleLlamaEngineError::LockPoisoned { operation: "create session" })?;
        let sid = state.next_session_id;
        state.next_session_id += 1;
        state.sessions.insert(sid, SessionState { tokens: Vec::new() });
        Ok(sid)
    }

    /// End a session and release its KV cache.
    pub async fn end_session(&self, session_id: SessionId) -> Result<(), EngineError> {
        let mut state = self
            .inner
            .write()
            .map_err(|_| CandleLlamaEngineError::LockPoisoned { operation: "end session" })?;
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
            self.run_inference_blocking(prompt, max_tokens, session_id).await
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
                    return Err(CandleLlamaEngineError::InferenceStreamError { message: e }.into());
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
        use candle_core::Device;
        use candle_transformers::generation::Sampling;

        let send = |chunk: StreamChunk| tx.blocking_send(chunk).is_ok();

        // ── Setup phase: single write lock to tokenize + resolve session prefix ──
        //
        // `ModelWeights::forward` takes `&mut self`, so write access is required
        // for the generation loop anyway.  Doing setup under the same write lock
        // eliminates the lock-upgrade race that existed when read and write locks
        // were acquired separately.
        let (mut all_tokens, mut logits_processor, eos_token) = {
            let state = match self.inner.write() {
                Ok(s) => s,
                Err(_) => {
                    send(StreamChunk::Error("lock poisoned".into()));
                    return;
                }
            };

            let tokenizer_ref = match (&state.model, &state.tokenizer) {
                (Some(_), Some(t)) => t,
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

            // Apply incremental-input delta if the session already has a cached prefix.
            if let Some(sess) = state.sessions.get(&session_id) {
                let cached = &sess.tokens;
                if tokens.starts_with(cached) && tokens.len() > cached.len() {
                    tokens = tokens[cached.len()..].to_vec();
                }
            }

            let eos_token = tokenizer_ref
                .token_to_id("</s>")
                .or_else(|| tokenizer_ref.token_to_id("<|end_of_text|>"))
                .or_else(|| tokenizer_ref.token_to_id("<|endoftext|>"))
                .unwrap_or(2);

            let lp = LogitsProcessor::from_sampling(state.seed, Sampling::ArgMax);
            (tokens, lp, eos_token)
        };

        // ── Generation loop ───────────────────────────────────────────────────
        //
        // Each iteration acquires the write lock once for:
        //   1. model.forward() (requires &mut model)
        //   2. EOS check
        //   3. Token decode
        // The lock is released before send() so channel pressure cannot cause
        // a deadlock while the lock is held.
        let device = Device::Cpu;
        let max_new_tokens = max_tokens.min(4096);
        let mut forward_pos = all_tokens.len().saturating_sub(1);

        for _ in 0..max_new_tokens {
            let input = match CandleTensor::new(all_tokens.as_slice(), &device)
                .and_then(|t| t.unsqueeze(0))
            {
                Ok(t) => t,
                Err(e) => {
                    send(StreamChunk::Error(format!("tensor error: {e}")));
                    return;
                }
            };

            // Acquire write lock once per token for forward + decode.
            let (next_token, token_text) = {
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
                let logits = match model.forward(&input, forward_pos) {
                    Ok(l) => l,
                    Err(e) => {
                        send(StreamChunk::Error(format!("forward pass error: {e}")));
                        return;
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
                let token_text = state
                    .tokenizer
                    .as_ref()
                    .and_then(|t| t.id_to_token(next_token).map(|s| s.replace('▁', " ")))
                    .unwrap_or_default();
                (next_token, token_text)
            };

            if next_token == eos_token {
                break;
            }

            all_tokens.push(next_token);
            forward_pos += 1;

            if !send(StreamChunk::Token(token_text)) {
                break;
            }
        }

        // Update session token cache.
        if let Ok(mut state) = self.inner.write()
            && let Some(sess) = state.sessions.get_mut(&session_id)
        {
            sess.tokens = all_tokens;
        }

        send(StreamChunk::Done);
    }
}
