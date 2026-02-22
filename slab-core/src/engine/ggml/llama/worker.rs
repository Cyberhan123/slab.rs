use slab_llama::{LlamaBatch, LlamaContext, LlamaModel, LlamaSeqId, LlamaToken};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use super::{GGMLLlamaEngineError, SessionId, StreamChunk};

// ── Internal channel protocol ─────────────────────────────────────────────────

/// Commands forwarded from the master worker to a specific inference worker.
pub(super) enum WorkerCommand {
    CreateSession {
        session_id: SessionId,
        reply_tx: oneshot::Sender<Result<(), GGMLLlamaEngineError>>,
    },
    AppendInput {
        session_id: SessionId,
        text_delta: String,
        reply_tx: oneshot::Sender<Result<(), GGMLLlamaEngineError>>,
    },
    GenerateStream {
        session_id: SessionId,
        max_new_tokens: usize,
        stream_tx: mpsc::Sender<StreamChunk>,
        reply_tx: oneshot::Sender<Result<(), GGMLLlamaEngineError>>,
    },
    EndSession {
        session_id: SessionId,
        reply_tx: oneshot::Sender<Result<(), GGMLLlamaEngineError>>,
    },
    Cancel {
        session_id: SessionId,
        reply_tx: oneshot::Sender<Result<(), GGMLLlamaEngineError>>,
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

pub(super) struct InferenceWorkerState {
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
    pub(super) fn new(
        worker_id: usize,
        model: Arc<LlamaModel>,
        ctx: LlamaContext,
        cmd_rx: mpsc::Receiver<WorkerCommand>,
    ) -> Self {
        Self {
            worker_id,
            model,
            ctx,
            sessions: HashMap::new(),
            next_seq_id: 0,
            free_seq_ids: Vec::new(),
            cmd_rx,
        }
    }

    fn handle_command(&mut self, cmd: WorkerCommand) {
        match cmd {
            WorkerCommand::CreateSession { session_id, reply_tx } => {
                // Prefer a recycled sequence ID; only mint a new one when the
                // free-list is empty, to keep the seq_id space bounded.
                let seq_id = self.free_seq_ids.pop().unwrap_or_else(|| {
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

            WorkerCommand::AppendInput {
                session_id,
                text_delta,
                reply_tx,
            } => match self.sessions.get_mut(&session_id) {
                None => {
                    let _ = reply_tx.send(Err(GGMLLlamaEngineError::SessionNotFound { session_id }));
                }
                Some(session) => {
                    // Tokenize the delta (no BOS, parse special tokens).
                    let result = self
                        .model
                        .tokenize(&text_delta, false, true)
                        .map(|tokens| {
                            session.pending_tokens.extend(tokens);
                        })
                        .map_err(|source| GGMLLlamaEngineError::TokenizeFailed {
                            source: source.into(),
                        });
                    let _ = reply_tx.send(result);
                }
            },

            WorkerCommand::GenerateStream {
                session_id,
                max_new_tokens,
                stream_tx,
                reply_tx,
            } => match self.sessions.get_mut(&session_id) {
                None => {
                    let _ = reply_tx.send(Err(GGMLLlamaEngineError::SessionNotFound { session_id }));
                }
                Some(session) => {
                    session.stream_tx = Some(stream_tx);
                    session.remaining_tokens = max_new_tokens;
                    session.cancelled = false;
                    let _ = reply_tx.send(Ok(()));
                }
            },

            WorkerCommand::EndSession {
                session_id,
                reply_tx,
            } => match self.sessions.remove(&session_id) {
                None => {
                    let _ = reply_tx.send(Err(GGMLLlamaEngineError::SessionNotFound { session_id }));
                }
                Some(session) => {
                    // Release KV cache entries for this sequence only.
                    self.ctx.kv_cache_seq_rm(session.seq_id, 0, i32::MAX);
                    // Return the sequence ID to the free-list so it can be
                    // reused by a future session without exhausting the ID space.
                    self.free_seq_ids.push(session.seq_id);
                    let _ = reply_tx.send(Ok(()));
                }
            },

            WorkerCommand::Cancel {
                session_id,
                reply_tx,
            } => match self.sessions.get_mut(&session_id) {
                None => {
                    let _ = reply_tx.send(Err(GGMLLlamaEngineError::SessionNotFound { session_id }));
                }
                Some(session) => {
                    session.cancelled = true;
                    let _ = reply_tx.send(Ok(()));
                }
            },
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
    pub(super) fn run(mut self) {
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
                    None => return,
                }
            }
        }
    }
}