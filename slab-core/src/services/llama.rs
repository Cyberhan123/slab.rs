use crate::services;
use slab_llama::{
    LlamaBatch, LlamaContext, LlamaContextParams, LlamaModel, LlamaModelParams, Llama,
    LlamaSeqId, LlamaToken, SamplerChainBuilder,
};
use std::collections::HashMap;
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tracing::info;

/// A unique identifier for an inference session.
pub type SessionId = u64;

struct LlamaGlobal {
    service: Arc<LlamaService>,
    lib_path: PathBuf,
}

static INSTANCE: OnceLock<RwLock<Option<LlamaGlobal>>> = OnceLock::new();

#[derive(Debug, Error)]
pub enum LlamaServiceError {
    #[error(
        "LlamaService already initialized with different library path: {existing} (requested: {requested})"
    )]
    LibraryPathMismatch { existing: PathBuf, requested: PathBuf },

    #[error("LlamaService global storage not initialized")]
    GlobalStorageNotInitialized,

    #[error("LlamaService instance not initialized")]
    InstanceNotInitialized,

    #[error("Lock poisoned while trying to {operation}")]
    LockPoisoned { operation: &'static str },

    #[error("Model path contains invalid UTF-8")]
    InvalidModelPathUtf8,

    #[error("Llama model not loaded")]
    ModelNotLoaded,

    #[error("Failed to canonicalize llama library path: {path}")]
    CanonicalizeLibraryPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to initialize llama dynamic library at: {path}")]
    InitializeDynamicLibrary {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to load llama model from: {model_path}")]
    LoadModel {
        model_path: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to create llama context")]
    CreateContext {
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to tokenize prompt")]
    TokenizeFailed {
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to decode batch")]
    DecodeFailed {
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to convert token to piece")]
    TokenToPieceFailed {
        #[source]
        source: anyhow::Error,
    },

    #[error("Session {session_id} not found")]
    SessionNotFound { session_id: SessionId },

    #[error("Inference worker shut down unexpectedly")]
    WorkerShutdown,

    #[error("Failed to spawn inference worker thread")]
    SpawnWorkerFailed {
        #[source]
        source: std::io::Error,
    },
}

/// Holds the loaded model and its inference context.
#[derive(Debug)]
struct LlamaState {
    model: LlamaModel,
    ctx: LlamaContext,
}

#[derive(Debug)]
pub struct LlamaService {
    instance: Arc<Llama>,
    state: Arc<Mutex<Option<LlamaState>>>,
}

// SAFETY: LlamaService is only accessed through Arc<Mutex<...>> for mutable state.
// The `instance: Arc<Llama>` field wraps a dynamically loaded library handle which is
// immutable after creation (models and contexts are created from it, not mutated).
// All mutable inference state is guarded by the `state: Arc<Mutex<...>>` field.
unsafe impl Send for LlamaService {}
unsafe impl Sync for LlamaService {}

impl LlamaService {
    fn resolve_lib_path<P: AsRef<Path>>(path: P) -> Result<PathBuf, services::ServiceError> {
        let llama_lib_name = format!("{}llama{}", DLL_PREFIX, DLL_SUFFIX);

        let mut lib_path = path.as_ref().to_path_buf();
        if lib_path.file_name() != Some(OsStr::new(&llama_lib_name)) {
            lib_path.push(&llama_lib_name);
        }

        std::fs::canonicalize(&lib_path).map_err(|source| {
            LlamaServiceError::CanonicalizeLibraryPath {
                path: lib_path,
                source,
            }
            .into()
        })
    }

    fn build_service(normalized_path: &Path) -> Result<Self, services::ServiceError> {
        info!("current llama path is: {}", normalized_path.display());
        let llama = Llama::new(normalized_path).map_err(|source| {
            LlamaServiceError::InitializeDynamicLibrary {
                path: normalized_path.to_path_buf(),
                source: source.into(),
            }
        })?;

        llama.backend_init();

        Ok(Self {
            instance: Arc::new(llama),
            state: Arc::new(Mutex::new(None)),
        })
    }

    pub fn init<P: AsRef<Path>>(path: P) -> Result<Arc<Self>, services::ServiceError> {
        let normalized_path = Self::resolve_lib_path(path)?;
        let global_lock = INSTANCE.get_or_init(|| RwLock::new(None));

        {
            let read_guard = global_lock
                .read()
                .map_err(|_| LlamaServiceError::LockPoisoned {
                    operation: "read llama global state",
                })?;
            if let Some(global) = read_guard.as_ref() {
                if global.lib_path != normalized_path {
                    return Err(LlamaServiceError::LibraryPathMismatch {
                        existing: global.lib_path.clone(),
                        requested: normalized_path.clone(),
                    }
                    .into());
                }
                return Ok(global.service.clone());
            }
        }

        let service = Arc::new(Self::build_service(&normalized_path)?);
        let mut write_guard = global_lock
            .write()
            .map_err(|_| LlamaServiceError::LockPoisoned {
                operation: "write llama global state",
            })?;

        if let Some(global) = write_guard.as_ref() {
            if global.lib_path != normalized_path {
                return Err(LlamaServiceError::LibraryPathMismatch {
                    existing: global.lib_path.clone(),
                    requested: normalized_path.clone(),
                }
                .into());
            }
            return Ok(global.service.clone());
        }

        *write_guard = Some(LlamaGlobal {
            service: service.clone(),
            lib_path: normalized_path,
        });

        Ok(service)
    }

    pub fn reload<P: AsRef<Path>>(path: P) -> Result<Arc<Self>, services::ServiceError> {
        let normalized_path = Self::resolve_lib_path(path)?;
        let service = Arc::new(Self::build_service(&normalized_path)?);
        let global_lock = INSTANCE.get_or_init(|| RwLock::new(None));
        let mut write_guard = global_lock
            .write()
            .map_err(|_| LlamaServiceError::LockPoisoned {
                operation: "write llama global state",
            })?;

        let previous = write_guard
            .as_ref()
            .map(|g| g.lib_path.display().to_string())
            .unwrap_or_else(|| "<uninitialized>".to_string());

        *write_guard = Some(LlamaGlobal {
            service: service.clone(),
            lib_path: normalized_path.clone(),
        });

        info!(
            "llama service reloaded: {} -> {}",
            previous,
            normalized_path.display()
        );

        Ok(service)
    }

    pub fn current() -> Result<Arc<Self>, services::ServiceError> {
        let global_lock = INSTANCE
            .get()
            .ok_or(LlamaServiceError::GlobalStorageNotInitialized)?;
        let read_guard = global_lock
            .read()
            .map_err(|_| LlamaServiceError::LockPoisoned {
                operation: "read llama global state",
            })?;
        read_guard
            .as_ref()
            .map(|global| global.service.clone())
            .ok_or(LlamaServiceError::InstanceNotInitialized.into())
    }

    /// Load a model from a GGUF file and create an inference context.
    ///
    /// Any previously loaded model and context are replaced.
    pub fn load_model<P: AsRef<Path>>(
        &self,
        path_to_model: P,
        model_params: LlamaModelParams,
        ctx_params: LlamaContextParams,
    ) -> Result<(), services::ServiceError> {
        let mut state_lock =
            self.state
                .lock()
                .map_err(|_| LlamaServiceError::LockPoisoned {
                    operation: "lock llama state",
                })?;
        *state_lock = None;

        let path = path_to_model
            .as_ref()
            .to_str()
            .ok_or(LlamaServiceError::InvalidModelPathUtf8)?;

        let model = self
            .instance
            .load_model_from_file(path, model_params)
            .map_err(|source| LlamaServiceError::LoadModel {
                model_path: path.to_string(),
                source: source.into(),
            })?;

        let ctx = model
            .new_context(ctx_params)
            .map_err(|source| LlamaServiceError::CreateContext {
                source: source.into(),
            })?;

        *state_lock = Some(LlamaState { model, ctx });
        Ok(())
    }

    /// Generate text from a prompt.
    ///
    /// Runs autoregressive decoding until `max_tokens` new tokens are produced
    /// or an end-of-generation token is sampled.  Each call resets the KV cache
    /// so generations are independent.
    pub fn generate(
        &self,
        prompt: &str,
        max_tokens: usize,
    ) -> Result<String, services::ServiceError> {
        let mut state_lock =
            self.state
                .lock()
                .map_err(|_| LlamaServiceError::LockPoisoned {
                    operation: "lock llama state",
                })?;

        let state = state_lock
            .as_mut()
            .ok_or(LlamaServiceError::ModelNotLoaded)?;

        // Start each generation from a clean KV cache.
        state.ctx.kv_cache_clear();

        // Build a fresh sampler chain for this generation.
        let mut sampler = SamplerChainBuilder::new(self.instance.lib_arc()).build();

        // Tokenize the prompt (add BOS, parse special tokens).
        let tokens = state
            .model
            .tokenize(prompt, true, true)
            .map_err(|source| LlamaServiceError::TokenizeFailed {
                source: source.into(),
            })?;

        if tokens.is_empty() {
            return Ok(String::new());
        }

        // Fill the initial batch with all prompt tokens.
        // Only the last token needs logits (sampling happens there).
        let mut batch = LlamaBatch::new(tokens.len());
        for (i, &token) in tokens.iter().enumerate() {
            batch
                .add(token, i as i32, &[0], i == tokens.len() - 1)
                .map_err(|source| LlamaServiceError::DecodeFailed {
                    source: source.into(),
                })?;
        }

        // Decode the prompt in one shot.
        state
            .ctx
            .decode(&mut batch)
            .map_err(|source| LlamaServiceError::DecodeFailed {
                source: source.into(),
            })?;

        let mut n_cur = tokens.len() as i32;
        let mut output = String::new();

        for _ in 0..max_tokens {
            // Sample from the last logit in the batch (-1 = last position).
            let new_token = sampler.sample(&mut state.ctx, -1);
            sampler.accept(new_token);

            if state.model.token_is_eog(new_token) {
                break;
            }

            let piece = state
                .model
                .token_to_piece(new_token, true)
                .map_err(|source| LlamaServiceError::TokenToPieceFailed {
                    source: source.into(),
                })?;
            output.push_str(&piece);

            // Decode the newly sampled token to advance the context.
            batch.clear();
            batch
                .add(new_token, n_cur, &[0], true)
                .map_err(|source| LlamaServiceError::DecodeFailed {
                    source: source.into(),
                })?;

            state
                .ctx
                .decode(&mut batch)
                .map_err(|source| LlamaServiceError::DecodeFailed {
                    source: source.into(),
                })?;

            n_cur += 1;
        }

        Ok(output)
    }
}

// ── Multi-worker inference engine ─────────────────────────────────────────────

/// A chunk of streaming output from the inference engine.
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// A piece of generated text.
    Token(String),
    /// Generation completed normally.
    Done,
    /// Generation terminated due to an error.
    Error(String),
}

/// A handle to a streaming generation response.
///
/// Yields [`StreamChunk`] items as tokens are produced.  The stream ends
/// with [`StreamChunk::Done`] or [`StreamChunk::Error`].
pub type StreamHandle = mpsc::Receiver<StreamChunk>;

// ── Internal channel protocol ─────────────────────────────────────────────────

/// Commands forwarded from the master worker to a specific inference worker.
enum WorkerCommand {
    CreateSession {
        session_id: SessionId,
        reply_tx: oneshot::Sender<Result<(), LlamaServiceError>>,
    },
    AppendInput {
        session_id: SessionId,
        text_delta: String,
        reply_tx: oneshot::Sender<Result<(), LlamaServiceError>>,
    },
    GenerateStream {
        session_id: SessionId,
        max_new_tokens: usize,
        stream_tx: mpsc::Sender<StreamChunk>,
        reply_tx: oneshot::Sender<Result<(), LlamaServiceError>>,
    },
    EndSession {
        session_id: SessionId,
        reply_tx: oneshot::Sender<Result<(), LlamaServiceError>>,
    },
    Cancel {
        session_id: SessionId,
        reply_tx: oneshot::Sender<Result<(), LlamaServiceError>>,
    },
}

/// Commands sent by API callers to the global ingress queue (master worker).
enum GlobalCommand {
    CreateSession {
        reply_tx: oneshot::Sender<Result<SessionId, LlamaServiceError>>,
    },
    AppendInput {
        session_id: SessionId,
        text_delta: String,
        reply_tx: oneshot::Sender<Result<(), LlamaServiceError>>,
    },
    GenerateStream {
        session_id: SessionId,
        max_new_tokens: usize,
        stream_tx: mpsc::Sender<StreamChunk>,
        reply_tx: oneshot::Sender<Result<(), LlamaServiceError>>,
    },
    EndSession {
        session_id: SessionId,
        reply_tx: oneshot::Sender<Result<(), LlamaServiceError>>,
    },
    Cancel {
        session_id: SessionId,
        reply_tx: oneshot::Sender<Result<(), LlamaServiceError>>,
    },
}

// ── Per-session state (inside an inference worker) ────────────────────────────

struct SessionState {
    /// Sequence ID in the KV cache for this session.
    seq_id: LlamaSeqId,
    /// Number of tokens already decoded into the KV cache for this sequence.
    n_past: i32,
    /// Tokens from the latest `append_input` delta, waiting to be prefilled.
    pending_tokens: Vec<LlamaToken>,
    /// Per-session sampler (wrapped in Option so it can be temporarily moved out
    /// during batch sampling without conflicting borrows).
    sampler: Option<slab_llama::LlamaSampler>,
    // ── Active generation state ──────────────────────────────────────────────
    /// Channel to send generated tokens to the caller.
    stream_tx: Option<mpsc::Sender<StreamChunk>>,
    /// Remaining token budget for the current generation.
    remaining_tokens: usize,
    /// The most-recently sampled token, ready to be decoded in the next batch
    /// (and whose text has already been forwarded to the stream).
    last_token: Option<LlamaToken>,
    /// Set to `true` by a `Cancel` command; generation stops at the next step.
    cancelled: bool,
}

// ── Inference worker ──────────────────────────────────────────────────────────

struct InferenceWorkerState {
    #[allow(dead_code)]
    worker_id: usize,
    model: Arc<LlamaModel>,
    ctx: LlamaContext,
    sessions: HashMap<SessionId, SessionState>,
    /// Monotonically increasing counter used to mint fresh sequence IDs when the
    /// free-list is empty.
    next_seq_id: LlamaSeqId,
    /// Pool of sequence IDs freed by `end_session` that can be reused.
    ///
    /// Reusing freed IDs keeps the seq_id space bounded even when many sessions
    /// are created and destroyed over the worker's lifetime.
    free_seq_ids: Vec<LlamaSeqId>,
    cmd_rx: mpsc::Receiver<WorkerCommand>,
}

impl InferenceWorkerState {
    fn handle_command(&mut self, cmd: WorkerCommand) {
        match cmd {
            WorkerCommand::CreateSession { session_id, reply_tx } => {
                // Prefer a recycled sequence ID; only mint a new one when the
                // free-list is empty, to keep the seq_id space bounded.
                let seq_id = self
                    .free_seq_ids
                    .pop()
                    .unwrap_or_else(|| {
                        let id = self.next_seq_id;
                        self.next_seq_id += 1;
                        id
                    });
                let sampler = self.model.new_sampler();
                self.sessions.insert(
                    session_id,
                    SessionState {
                        seq_id,
                        n_past: 0,
                        pending_tokens: Vec::new(),
                        sampler: Some(sampler),
                        stream_tx: None,
                        remaining_tokens: 0,
                        last_token: None,
                        cancelled: false,
                    },
                );
                let _ = reply_tx.send(Ok(()));
            }

            WorkerCommand::AppendInput { session_id, text_delta, reply_tx } => {
                match self.sessions.get_mut(&session_id) {
                    None => {
                        let _ = reply_tx
                            .send(Err(LlamaServiceError::SessionNotFound { session_id }));
                    }
                    Some(session) => {
                        // Tokenize the delta (no BOS, parse special tokens).
                        let result = self
                            .model
                            .tokenize(&text_delta, false, true)
                            .map(|tokens| {
                                session.pending_tokens.extend(tokens);
                            })
                            .map_err(|source| LlamaServiceError::TokenizeFailed {
                                source: source.into(),
                            });
                        let _ = reply_tx.send(result);
                    }
                }
            }

            WorkerCommand::GenerateStream {
                session_id,
                max_new_tokens,
                stream_tx,
                reply_tx,
            } => {
                match self.sessions.get_mut(&session_id) {
                    None => {
                        let _ = reply_tx
                            .send(Err(LlamaServiceError::SessionNotFound { session_id }));
                    }
                    Some(session) => {
                        session.stream_tx = Some(stream_tx);
                        session.remaining_tokens = max_new_tokens;
                        session.cancelled = false;
                        let _ = reply_tx.send(Ok(()));
                    }
                }
            }

            WorkerCommand::EndSession { session_id, reply_tx } => {
                match self.sessions.remove(&session_id) {
                    None => {
                        let _ = reply_tx
                            .send(Err(LlamaServiceError::SessionNotFound { session_id }));
                    }
                    Some(session) => {
                        // Release KV cache entries for this sequence only.
                        self.ctx.kv_cache_seq_rm(session.seq_id, 0, i32::MAX);
                        // Return the sequence ID to the free-list so it can be
                        // reused by a future session without exhausting the ID space.
                        self.free_seq_ids.push(session.seq_id);
                        let _ = reply_tx.send(Ok(()));
                    }
                }
            }

            WorkerCommand::Cancel { session_id, reply_tx } => {
                match self.sessions.get_mut(&session_id) {
                    None => {
                        let _ = reply_tx
                            .send(Err(LlamaServiceError::SessionNotFound { session_id }));
                    }
                    Some(session) => {
                        session.cancelled = true;
                        let _ = reply_tx.send(Ok(()));
                    }
                }
            }
        }
    }

    /// Returns `true` when there is inference work queued for at least one session.
    ///
    /// Work exists when a session has active generation **and** either:
    /// - pending prefill tokens (from `append_input`), or
    /// - a previously sampled token that needs to be decoded (continuing generation).
    fn has_work(&self) -> bool {
        self.sessions.values().any(|s| {
            !s.cancelled
                && s.stream_tx.is_some()
                && s.remaining_tokens > 0
                && (!s.pending_tokens.is_empty() || s.last_token.is_some())
        })
    }

    /// Execute one continuous-batching step across all ready sessions.
    ///
    /// The step is divided into four phases:
    /// 1. **Batch building** – collect prefill tokens and generation tokens.
    /// 2. **Decode** – call `llama_decode` once for the combined batch.
    /// 3. **Position update** – advance `n_past` counters.
    /// 4. **Sampling** – sample the next token per session and emit to streams.
    fn run_inference_step(&mut self) {
        let batch_capacity = self.ctx.n_batch() as usize;
        let mut batch = LlamaBatch::new(batch_capacity);
        // Ordered list of session_ids that requested logits in this batch.
        let mut logit_owners: Vec<SessionId> = Vec::new();
        // Sessions that were prefilled in this step: session_id → token count.
        let mut prefill_counts: HashMap<SessionId, usize> = HashMap::new();
        // Sessions that advanced via a generation decode in this step.
        let mut gen_sessions: Vec<SessionId> = Vec::new();

        let session_ids: Vec<SessionId> = self.sessions.keys().copied().collect();

        for &session_id in &session_ids {
            let session = self.sessions.get_mut(&session_id).unwrap();

            // Handle cancellation before building the batch.
            if session.cancelled {
                if let Some(tx) = session.stream_tx.take() {
                    let _ = tx.blocking_send(StreamChunk::Done);
                }
                session.remaining_tokens = 0;
                session.last_token = None;
                continue;
            }

            // Only process sessions with active generation.
            if session.stream_tx.is_none() || session.remaining_tokens == 0 {
                continue;
            }

            if !session.pending_tokens.is_empty() {
                // ── Prefill phase ────────────────────────────────────────────
                let n = session.pending_tokens.len();
                // Skip if there is no room in the current batch.
                if (batch.n_tokens() as usize) + n > batch_capacity {
                    continue;
                }
                for (i, &token) in session.pending_tokens.iter().enumerate() {
                    let is_last = i == n - 1;
                    // Request logits only for the final prefill token so we can
                    // sample the first generated token immediately.
                    // INVARIANT: capacity is verified above; `add` cannot return
                    // BatchFull here.
                    batch
                        .add(token, session.n_past + i as i32, &[session.seq_id], is_last)
                        .expect("batch capacity verified; add cannot fail");
                    if is_last {
                        logit_owners.push(session_id);
                    }
                }
                prefill_counts.insert(session_id, n);
            } else if let Some(last_token) = session.last_token {
                // ── Generation step ──────────────────────────────────────────
                if (batch.n_tokens() as usize) < batch_capacity {
                    // INVARIANT: capacity is verified by the condition above.
                    batch
                        .add(last_token, session.n_past, &[session.seq_id], true)
                        .expect("batch capacity verified; add cannot fail");
                    logit_owners.push(session_id);
                    gen_sessions.push(session_id);
                }
            }
        }

        if batch.n_tokens() == 0 {
            return;
        }

        // ── Decode ────────────────────────────────────────────────────────────
        if let Err(e) = self.ctx.decode(&mut batch) {
            let msg = e.to_string();
            for s in self.sessions.values_mut() {
                if let Some(tx) = s.stream_tx.take() {
                    let _ = tx.blocking_send(StreamChunk::Error(msg.clone()));
                    s.remaining_tokens = 0;
                }
            }
            return;
        }

        // ── Position update ───────────────────────────────────────────────────
        for (&session_id, &count) in &prefill_counts {
            let s = self.sessions.get_mut(&session_id).unwrap();
            s.n_past += count as i32;
            s.pending_tokens.clear();
        }
        for &session_id in &gen_sessions {
            let s = self.sessions.get_mut(&session_id).unwrap();
            s.n_past += 1;
            s.last_token = None;
        }

        // ── Sampling ─────────────────────────────────────────────────────────
        for (logit_idx, &session_id) in logit_owners.iter().enumerate() {
            // Temporarily take the sampler out to avoid a simultaneous mutable
            // borrow of `self.sessions` and `self.ctx`.
            let mut sampler = self
                .sessions
                .get_mut(&session_id)
                .unwrap()
                .sampler
                .take()
                .unwrap();

            let token = sampler.sample(&mut self.ctx, logit_idx as i32);
            sampler.accept(token);

            // Restore the sampler before any further session mutation.
            self.sessions.get_mut(&session_id).unwrap().sampler = Some(sampler);

            let is_eog = self.model.token_is_eog(token);
            let session = self.sessions.get_mut(&session_id).unwrap();
            let remaining = session.remaining_tokens.saturating_sub(1);

            if is_eog || remaining == 0 {
                // Generation complete: optionally send the final piece, then Done.
                if let Some(tx) = session.stream_tx.take() {
                    if !is_eog {
                        if let Ok(piece) = self.model.token_to_piece(token, true) {
                            if !piece.is_empty() {
                                let _ = tx.blocking_send(StreamChunk::Token(piece));
                            }
                        }
                    }
                    let _ = tx.blocking_send(StreamChunk::Done);
                }
                session.remaining_tokens = 0;
                session.last_token = None;
            } else {
                // Emit the token piece to the caller and queue the token for the
                // next decode step.
                match self.model.token_to_piece(token, true) {
                    Ok(piece) => {
                        if let Some(tx) = &session.stream_tx {
                            match tx.blocking_send(StreamChunk::Token(piece)) {
                                Ok(()) => {
                                    session.remaining_tokens = remaining;
                                    session.last_token = Some(token);
                                }
                                Err(_) => {
                                    // Receiver was dropped; stop generation silently.
                                    session.stream_tx = None;
                                    session.remaining_tokens = 0;
                                    session.last_token = None;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if let Some(tx) = session.stream_tx.take() {
                            let _ = tx.blocking_send(StreamChunk::Error(e.to_string()));
                        }
                        session.remaining_tokens = 0;
                        session.last_token = None;
                    }
                }
            }
        }
    }

    /// Main loop for an inference worker thread.
    ///
    /// The loop alternates between draining incoming commands and executing a
    /// single continuous-batching inference step whenever work is available.
    fn run(mut self) {
        loop {
            // Drain all pending commands (non-blocking).
            loop {
                match self.cmd_rx.try_recv() {
                    Ok(cmd) => self.handle_command(cmd),
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => return,
                }
            }

            if self.has_work() {
                self.run_inference_step();
            } else {
                // No work available; block until the next command arrives to
                // avoid busy-waiting.
                match self.cmd_rx.blocking_recv() {
                    Some(cmd) => self.handle_command(cmd),
                    None => return, // Channel closed; shut down.
                }
            }
        }
    }
}

// ── Master worker ─────────────────────────────────────────────────────────────

/// Consumes the global ingress queue and routes commands to inference workers.
///
/// Maintains the `session_id → worker_id` mapping (session pinning).
struct MasterWorkerState {
    global_rx: mpsc::Receiver<GlobalCommand>,
    worker_txs: Vec<mpsc::Sender<WorkerCommand>>,
    /// Session-to-worker mapping (enforces session pinning).
    session_map: HashMap<SessionId, usize>,
    /// Round-robin counter for new-session assignment.
    next_worker: usize,
    /// Monotonically increasing counter for session IDs.
    next_session_id: u64,
}

impl MasterWorkerState {
    async fn run(mut self) {
        while let Some(cmd) = self.global_rx.recv().await {
            match cmd {
                GlobalCommand::CreateSession { reply_tx } => {
                    let session_id = self.next_session_id;
                    self.next_session_id += 1;
                    let worker_id = self.next_worker % self.worker_txs.len();
                    self.next_worker += 1;

                    let (ack_tx, ack_rx) = oneshot::channel();
                    if self.worker_txs[worker_id]
                        .send(WorkerCommand::CreateSession { session_id, reply_tx: ack_tx })
                        .await
                        .is_err()
                    {
                        let _ = reply_tx.send(Err(LlamaServiceError::WorkerShutdown));
                        continue;
                    }
                    match ack_rx.await {
                        Ok(Ok(())) => {
                            self.session_map.insert(session_id, worker_id);
                            let _ = reply_tx.send(Ok(session_id));
                        }
                        Ok(Err(e)) => {
                            let _ = reply_tx.send(Err(e));
                        }
                        Err(_) => {
                            let _ = reply_tx.send(Err(LlamaServiceError::WorkerShutdown));
                        }
                    }
                }

                GlobalCommand::AppendInput { session_id, text_delta, reply_tx } => {
                    match self.session_map.get(&session_id) {
                        None => {
                            let _ = reply_tx.send(Err(LlamaServiceError::SessionNotFound {
                                session_id,
                            }));
                        }
                        Some(&worker_id) => {
                            let (ack_tx, ack_rx) = oneshot::channel();
                            if self.worker_txs[worker_id]
                                .send(WorkerCommand::AppendInput {
                                    session_id,
                                    text_delta,
                                    reply_tx: ack_tx,
                                })
                                .await
                                .is_err()
                            {
                                let _ = reply_tx.send(Err(LlamaServiceError::WorkerShutdown));
                                continue;
                            }
                            match ack_rx.await {
                                Ok(r) => {
                                    let _ = reply_tx.send(r);
                                }
                                Err(_) => {
                                    let _ =
                                        reply_tx.send(Err(LlamaServiceError::WorkerShutdown));
                                }
                            }
                        }
                    }
                }

                GlobalCommand::GenerateStream {
                    session_id,
                    max_new_tokens,
                    stream_tx,
                    reply_tx,
                } => {
                    match self.session_map.get(&session_id) {
                        None => {
                            let _ = reply_tx.send(Err(LlamaServiceError::SessionNotFound {
                                session_id,
                            }));
                        }
                        Some(&worker_id) => {
                            let (ack_tx, ack_rx) = oneshot::channel();
                            if self.worker_txs[worker_id]
                                .send(WorkerCommand::GenerateStream {
                                    session_id,
                                    max_new_tokens,
                                    stream_tx,
                                    reply_tx: ack_tx,
                                })
                                .await
                                .is_err()
                            {
                                let _ = reply_tx.send(Err(LlamaServiceError::WorkerShutdown));
                                continue;
                            }
                            match ack_rx.await {
                                Ok(r) => {
                                    let _ = reply_tx.send(r);
                                }
                                Err(_) => {
                                    let _ =
                                        reply_tx.send(Err(LlamaServiceError::WorkerShutdown));
                                }
                            }
                        }
                    }
                }

                GlobalCommand::EndSession { session_id, reply_tx } => {
                    match self.session_map.get(&session_id).copied() {
                        None => {
                            let _ = reply_tx.send(Err(LlamaServiceError::SessionNotFound {
                                session_id,
                            }));
                        }
                        Some(worker_id) => {
                            let (ack_tx, ack_rx) = oneshot::channel();
                            if self.worker_txs[worker_id]
                                .send(WorkerCommand::EndSession {
                                    session_id,
                                    reply_tx: ack_tx,
                                })
                                .await
                                .is_err()
                            {
                                let _ = reply_tx.send(Err(LlamaServiceError::WorkerShutdown));
                                continue;
                            }
                            match ack_rx.await {
                                Ok(Ok(())) => {
                                    // Remove the mapping only after the worker has
                                    // confirmed it released the session's KV entries.
                                    self.session_map.remove(&session_id);
                                    let _ = reply_tx.send(Ok(()));
                                }
                                Ok(Err(e)) => {
                                    let _ = reply_tx.send(Err(e));
                                }
                                Err(_) => {
                                    let _ =
                                        reply_tx.send(Err(LlamaServiceError::WorkerShutdown));
                                }
                            }
                        }
                    }
                }

                GlobalCommand::Cancel { session_id, reply_tx } => {
                    match self.session_map.get(&session_id) {
                        None => {
                            let _ = reply_tx.send(Err(LlamaServiceError::SessionNotFound {
                                session_id,
                            }));
                        }
                        Some(&worker_id) => {
                            let (ack_tx, ack_rx) = oneshot::channel();
                            if self.worker_txs[worker_id]
                                .send(WorkerCommand::Cancel {
                                    session_id,
                                    reply_tx: ack_tx,
                                })
                                .await
                                .is_err()
                            {
                                let _ = reply_tx.send(Err(LlamaServiceError::WorkerShutdown));
                                continue;
                            }
                            match ack_rx.await {
                                Ok(r) => {
                                    let _ = reply_tx.send(r);
                                }
                                Err(_) => {
                                    let _ =
                                        reply_tx.send(Err(LlamaServiceError::WorkerShutdown));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Multi-worker inference engine with session-based KV reuse and streaming output.
///
/// # Architecture
///
/// ```text
/// Caller ──► global_tx ──► [Master Worker Task]
///                                │  session_id → worker_id
///                          ┌─────┴─────┐
///                          ▼           ▼
///                     [Worker 0]  [Worker N-1]
///                     LlamaCtx    LlamaCtx
///                     (batching)  (batching)
///                          │           │
///                     stream_tx   stream_tx  ──► Caller
/// ```
///
/// ## Key properties
/// - One `LlamaModel` (weights) is shared across all workers via `Arc`.
/// - Each worker owns exactly one `LlamaContext` and is the only thread that
///   calls `decode` on it.
/// - Sessions are pinned to a worker for their lifetime (no migration).
/// - KV cache is never fully cleared; per-session cleanup uses
///   `kv_cache_seq_rm`.
#[derive(Clone)]
pub struct LlamaInferenceEngine {
    global_tx: mpsc::Sender<GlobalCommand>,
}

impl LlamaInferenceEngine {
    /// Start the inference engine.
    ///
    /// Spawns `num_workers` inference worker OS-threads (each with its own
    /// `LlamaContext`) and a master Tokio task that consumes the global queue.
    ///
    /// # Arguments
    /// * `num_workers` – number of parallel inference workers (≥ 1).
    /// * `model`       – shared model weights wrapped in `Arc`.
    /// * `ctx_params`  – context creation parameters cloned for every worker.
    ///
    /// # Shutdown
    /// The engine shuts down naturally when all [`LlamaInferenceEngine`] clones
    /// are dropped: the underlying `global_tx` sender is closed, which causes
    /// the master task to exit its `recv()` loop, which in turn drops all
    /// `worker_tx` senders, causing each inference worker thread to exit its
    /// `blocking_recv()` call.  No explicit `shutdown()` call is required.
    ///
    /// # Panics
    /// Panics if called outside of a Tokio runtime.
    pub fn start(
        num_workers: usize,
        model: Arc<LlamaModel>,
        ctx_params: LlamaContextParams,
    ) -> Result<Self, LlamaServiceError> {
        assert!(num_workers > 0, "num_workers must be > 0");

        let mut worker_txs: Vec<mpsc::Sender<WorkerCommand>> = Vec::with_capacity(num_workers);

        for worker_id in 0..num_workers {
            let (cmd_tx, cmd_rx) = mpsc::channel::<WorkerCommand>(128);
            worker_txs.push(cmd_tx);

            let ctx = model
                .new_context(ctx_params.clone())
                .map_err(|source| LlamaServiceError::CreateContext {
                    source: source.into(),
                })?;

            let worker_state = InferenceWorkerState {
                worker_id,
                model: Arc::clone(&model),
                ctx,
                sessions: HashMap::new(),
                next_seq_id: 0,
                free_seq_ids: Vec::new(),
                cmd_rx,
            };

            std::thread::Builder::new()
                .name(format!("llama-worker-{worker_id}"))
                .spawn(move || worker_state.run())
                .map_err(|source| LlamaServiceError::SpawnWorkerFailed { source })?;
        }

        let (global_tx, global_rx) = mpsc::channel::<GlobalCommand>(256);

        let master = MasterWorkerState {
            global_rx,
            worker_txs,
            session_map: HashMap::new(),
            next_worker: 0,
            next_session_id: 0,
        };

        tokio::spawn(master.run());

        Ok(Self { global_tx })
    }

    /// Create a new inference session.
    ///
    /// Returns the [`SessionId`] to use in subsequent API calls.
    pub async fn create_session(&self) -> Result<SessionId, LlamaServiceError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::CreateSession { reply_tx })
            .await
            .map_err(|_| LlamaServiceError::WorkerShutdown)?;
        reply_rx.await.map_err(|_| LlamaServiceError::WorkerShutdown)?
    }

    /// Append delta text to the session's input buffer.
    ///
    /// The text is tokenized and queued for prefilling.  Call this before
    /// [`Self::generate_stream`] to populate the context with the new turn's
    /// prompt.
    pub async fn append_input(
        &self,
        session_id: SessionId,
        text_delta: String,
    ) -> Result<(), LlamaServiceError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::AppendInput { session_id, text_delta, reply_tx })
            .await
            .map_err(|_| LlamaServiceError::WorkerShutdown)?;
        reply_rx.await.map_err(|_| LlamaServiceError::WorkerShutdown)?
    }

    /// Start streaming generation for a session.
    ///
    /// Returns a [`StreamHandle`] that receives [`StreamChunk`] items as the
    /// inference worker produces them.  The stream is closed by the worker
    /// after [`StreamChunk::Done`] or [`StreamChunk::Error`].
    ///
    /// **The caller must drive the returned receiver** (i.e. call `.recv()` in
    /// a loop) to avoid blocking the inference worker's backpressure path.
    ///
    /// # Note
    /// Call [`Self::append_input`] at least once before calling this method so
    /// that the session has pending tokens for prefilling.
    ///
    /// If called while a previous generation is still in progress for the same
    /// session, the previous generation is implicitly cancelled: the old stream
    /// sender is replaced by the new one and the old [`StreamHandle`] will
    /// receive no further messages (it will block on `recv` indefinitely unless
    /// the caller drops it).  Use [`Self::cancel_generate`] first if you need
    /// an explicit `Done` on the previous stream.
    pub async fn generate_stream(
        &self,
        session_id: SessionId,
        max_new_tokens: usize,
    ) -> Result<StreamHandle, LlamaServiceError> {
        let (stream_tx, stream_rx) = mpsc::channel::<StreamChunk>(64);
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::GenerateStream {
                session_id,
                max_new_tokens,
                stream_tx,
                reply_tx,
            })
            .await
            .map_err(|_| LlamaServiceError::WorkerShutdown)?;
        reply_rx.await.map_err(|_| LlamaServiceError::WorkerShutdown)??;
        Ok(stream_rx)
    }

    /// End a session, releasing its KV-cache entries.
    ///
    /// Uses `kv_cache_seq_rm` internally so other sessions' KV data is
    /// unaffected.
    pub async fn end_session(&self, session_id: SessionId) -> Result<(), LlamaServiceError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::EndSession { session_id, reply_tx })
            .await
            .map_err(|_| LlamaServiceError::WorkerShutdown)?;
        reply_rx.await.map_err(|_| LlamaServiceError::WorkerShutdown)?
    }

    /// Cancel the active generation for a session without ending the session.
    ///
    /// The KV cache is preserved, and a new [`Self::generate_stream`] call can
    /// be made after appending more input.
    ///
    /// # KV cache consistency note
    /// If a token has already been sampled and emitted to the stream but not yet
    /// decoded into the KV cache (i.e. the worker loop is between the sampling
    /// and the next decode step), cancellation discards that pending token.
    /// This leaves the KV cache one token behind the text that was already sent
    /// to the caller.  To continue a conversation from the exact emitted text,
    /// re-append that final text delta with [`Self::append_input`] before
    /// calling [`Self::generate_stream`] again.
    pub async fn cancel_generate(
        &self,
        session_id: SessionId,
    ) -> Result<(), LlamaServiceError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::Cancel { session_id, reply_tx })
            .await
            .map_err(|_| LlamaServiceError::WorkerShutdown)?;
        reply_rx.await.map_err(|_| LlamaServiceError::WorkerShutdown)?
    }
}


mod test {
    use super::*;
    use crate::services::dylib::DylibService;
    use hf_hub::api::sync::Api;
    use std::path::PathBuf;
    use tokio;

    async fn ensure_llama_dir() -> PathBuf {
        let mut test_data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_data_path.push("../testdata");

        DylibService::new()
            .with_prefix_path(&test_data_path)
            .download_llama()
            .await
            .expect("Failed to download llama")
    }

    #[tokio::test]
    async fn test_llama_current_and_reload() {
        let llama_dir = ensure_llama_dir().await;

        let initial = LlamaService::init(llama_dir.as_path())
            .expect("failed to initialize llama service");
        let current = LlamaService::current().expect("failed to get current llama service");
        assert!(Arc::ptr_eq(&initial, &current));

        let reloaded = LlamaService::reload(llama_dir.as_path())
            .expect("failed to reload llama service");
        let current_after_reload =
            LlamaService::current().expect("failed to get current llama service after reload");

        assert!(Arc::ptr_eq(&reloaded, &current_after_reload));
        assert!(!Arc::ptr_eq(&initial, &reloaded));
    }

    #[tokio::test]
    async fn test_llama_generate() {
        let mut test_data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_data_path.push("../testdata");

        let llama_dir = ensure_llama_dir().await;

        let ls = LlamaService::init(llama_dir.as_path())
            .expect("failed to initialize llama service");

        let api = Api::new().expect("failed to init hf-api");
        let model_path = api
            .model("bartowski/Qwen2.5-0.5B-Instruct-GGUF".into())
            .get("Qwen2.5-0.5B-Instruct-Q4_K_M.gguf")
            .expect("failed to download model");

        ls.load_model(
            model_path.as_path(),
            LlamaModelParams::default(),
            LlamaContextParams::default(),
        )
        .expect("load model failed");

        let result = ls
            .generate("Hello, my name is", 64)
            .expect("generate failed");

        println!("Generated: {result}");
        assert!(!result.is_empty(), "expected non-empty output");
    }

    /// Resolve the canonical path of the llama shared library inside `dir`.
    fn llama_lib_path(dir: &std::path::Path) -> PathBuf {
        let lib_name = format!(
            "{}llama{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        );
        std::fs::canonicalize(dir.join(&lib_name))
            .expect("failed to canonicalize llama lib path")
    }

    /// Helper that loads the model from HF hub and starts a `LlamaInferenceEngine`.
    async fn make_engine(llama_dir: &std::path::Path) -> (Llama, Arc<LlamaModel>, LlamaInferenceEngine) {
        let api = Api::new().expect("failed to init hf-api");
        let model_path = api
            .model("bartowski/Qwen2.5-0.5B-Instruct-GGUF".into())
            .get("Qwen2.5-0.5B-Instruct-Q4_K_M.gguf")
            .expect("failed to download model");

        let lib_path = llama_lib_path(llama_dir);
        let llama = Llama::new(&lib_path).expect("failed to load llama lib");
        llama.backend_init();

        let model = Arc::new(
            llama
                .load_model_from_file(
                    model_path.to_str().expect("model path utf-8"),
                    LlamaModelParams::default(),
                )
                .expect("failed to load model"),
        );

        let engine = LlamaInferenceEngine::start(
            1,
            Arc::clone(&model),
            LlamaContextParams::default(),
        )
        .expect("failed to start engine");

        (llama, model, engine)
    }

    #[tokio::test]
    async fn test_engine_basic_generation() {
        let llama_dir = ensure_llama_dir().await;
        let (_llama, _model, engine) = make_engine(&llama_dir).await;

        let sid = engine.create_session().await.expect("create_session failed");
        engine
            .append_input(sid, "Hello, my name is".to_string())
            .await
            .expect("append_input failed");

        let mut stream = engine
            .generate_stream(sid, 32)
            .await
            .expect("generate_stream failed");

        let mut output = String::new();
        while let Some(chunk) = stream.recv().await {
            match chunk {
                StreamChunk::Token(text) => output.push_str(&text),
                StreamChunk::Done => break,
                StreamChunk::Error(e) => panic!("generation error: {e}"),
            }
        }

        println!("engine basic output: {output}");
        assert!(!output.is_empty(), "expected non-empty output");

        engine.end_session(sid).await.expect("end_session failed");
    }

    #[tokio::test]
    async fn test_engine_session_not_found() {
        let llama_dir = ensure_llama_dir().await;
        let (_llama, _model, engine) = make_engine(&llama_dir).await;

        let err = engine
            .append_input(9999, "hello".to_string())
            .await
            .unwrap_err();
        assert!(
            matches!(err, LlamaServiceError::SessionNotFound { session_id: 9999 }),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn test_engine_kv_reuse_multiturn() {
        let llama_dir = ensure_llama_dir().await;
        let (_llama, _model, engine) = make_engine(&llama_dir).await;

        let sid = engine.create_session().await.expect("create_session failed");

        // First turn.
        engine
            .append_input(sid, "What is 1+1?".to_string())
            .await
            .expect("first append failed");
        let mut stream = engine
            .generate_stream(sid, 16)
            .await
            .expect("first generate_stream failed");
        let mut turn1 = String::new();
        while let Some(chunk) = stream.recv().await {
            match chunk {
                StreamChunk::Token(t) => turn1.push_str(&t),
                StreamChunk::Done => break,
                StreamChunk::Error(e) => panic!("turn1 error: {e}"),
            }
        }
        assert!(!turn1.is_empty(), "first turn should produce output");

        // Second turn (KV reuse; n_past carries over from turn 1).
        engine
            .append_input(sid, " And what is 2+2?".to_string())
            .await
            .expect("second append failed");
        let mut stream2 = engine
            .generate_stream(sid, 16)
            .await
            .expect("second generate_stream failed");
        let mut turn2 = String::new();
        while let Some(chunk) = stream2.recv().await {
            match chunk {
                StreamChunk::Token(t) => turn2.push_str(&t),
                StreamChunk::Done => break,
                StreamChunk::Error(e) => panic!("turn2 error: {e}"),
            }
        }
        assert!(!turn2.is_empty(), "second turn should produce output");

        engine.end_session(sid).await.expect("end_session failed");
    }

    #[tokio::test]
    async fn test_engine_cancel_and_resume() {
        let llama_dir = ensure_llama_dir().await;
        let (_llama, _model, engine) = make_engine(&llama_dir).await;

        let sid = engine.create_session().await.expect("create_session failed");
        engine
            .append_input(sid, "Count to one hundred:".to_string())
            .await
            .expect("append failed");

        let mut stream = engine
            .generate_stream(sid, 512)
            .await
            .expect("generate_stream failed");

        // Read a few tokens then cancel.
        let mut tokens_before_cancel = 0usize;
        loop {
            match stream.recv().await {
                Some(StreamChunk::Token(_)) => {
                    tokens_before_cancel += 1;
                    if tokens_before_cancel >= 3 {
                        break;
                    }
                }
                Some(StreamChunk::Done) | None => break,
                Some(StreamChunk::Error(e)) => panic!("stream error: {e}"),
            }
        }

        engine.cancel_generate(sid).await.expect("cancel failed");

        // After cancellation the session should still be usable.
        engine
            .append_input(sid, " Just say done.".to_string())
            .await
            .expect("append after cancel failed");
        let mut stream2 = engine
            .generate_stream(sid, 8)
            .await
            .expect("generate after cancel failed");
        let mut post_cancel = String::new();
        while let Some(chunk) = stream2.recv().await {
            match chunk {
                StreamChunk::Token(t) => post_cancel.push_str(&t),
                StreamChunk::Done => break,
                StreamChunk::Error(e) => panic!("post-cancel error: {e}"),
            }
        }
        assert!(!post_cancel.is_empty(), "should generate after cancel");

        engine.end_session(sid).await.expect("end_session failed");
    }

    #[tokio::test]
    async fn test_engine_seq_id_reuse() {
        // Create and end many sessions to exercise the free-list path.
        let llama_dir = ensure_llama_dir().await;
        let (_llama, _model, engine) = make_engine(&llama_dir).await;

        for _ in 0..4 {
            let sid = engine.create_session().await.expect("create_session");
            engine
                .append_input(sid, "hi".to_string())
                .await
                .expect("append");
            let mut stream = engine
                .generate_stream(sid, 4)
                .await
                .expect("generate_stream");
            while let Some(chunk) = stream.recv().await {
                match chunk {
                    StreamChunk::Done => break,
                    StreamChunk::Error(e) => panic!("{e}"),
                    _ => {}
                }
            }
            engine.end_session(sid).await.expect("end_session");
        }
    }
}
