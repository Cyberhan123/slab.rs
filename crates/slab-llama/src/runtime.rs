use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tracing::warn;

use crate::{
    LlamaBatch, LlamaContext, LlamaContextParams, LlamaError, LlamaModel, LlamaSeqId, LlamaToken,
};

const fn default_flash_attn_enabled() -> bool {
    true
}

pub type SessionId = u64;

#[derive(Debug, Clone)]
pub struct LlamaSessionSnapshot {
    pub worker_id: usize,
    pub n_past: i32,
    pub state: Arc<[u8]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlamaLoadConfig {
    pub model_path: PathBuf,
    pub num_workers: usize,
    pub context_length: Option<u32>,
    #[serde(default = "default_flash_attn_enabled")]
    pub flash_attn: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gbnf: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LlamaInferenceParams {
    pub max_tokens: usize,
    pub session_key: Option<String>,
    pub gbnf: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub min_p: Option<f32>,
    pub repetition_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    #[serde(default)]
    pub ignore_eos: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<Value>,
    #[serde(default)]
    pub stop_sequences: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct LlamaLogitBias {
    pub token: LlamaToken,
    pub bias: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlamaStopInfo {
    pub finish_reason: String,
    pub stop_token_id: Option<LlamaToken>,
    pub stop_token_text: Option<String>,
    pub stop_token_kind: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlamaInferenceOutput {
    pub text: String,
    pub stop: Option<LlamaStopInfo>,
}

#[derive(Debug, Error)]
pub enum LlamaRuntimeError {
    #[error("Invalid llama worker count: {num_workers} (must be > 0)")]
    InvalidWorkerCount { num_workers: usize },

    #[error("Failed to create llama context")]
    CreateContext {
        #[source]
        source: LlamaError,
    },

    #[error("Failed to tokenize prompt")]
    TokenizeFailed {
        #[source]
        source: LlamaError,
    },

    #[error("Session {session_id} is not idle and cannot be snapshotted")]
    SessionNotIdle { session_id: SessionId },

    #[error("Failed to export llama session state")]
    SnapshotState {
        #[source]
        source: LlamaError,
    },

    #[error("Failed to restore llama session state")]
    RestoreState {
        #[source]
        source: LlamaError,
    },

    #[error("Session {session_id} not found")]
    SessionNotFound { session_id: SessionId },

    #[error("Session capacity exceeded: max concurrent sessions per worker is {max_sessions}")]
    SessionCapacityExceeded { max_sessions: usize },

    #[error("Inference worker shut down unexpectedly")]
    WorkerShutdown,

    #[error("Failed to spawn inference worker thread")]
    SpawnWorkerFailed {
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone)]
pub enum StreamChunk {
    Token(String),
    Stop(LlamaStopInfo),
    Done,
    Error(String),
}

pub type StreamHandle = mpsc::Receiver<StreamChunk>;

#[derive(Debug, Default)]
struct Utf8PieceBuffer {
    pending_bytes: Vec<u8>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Utf8FlushResult {
    text: Option<String>,
    dropped_incomplete_tail: bool,
}

impl Utf8PieceBuffer {
    fn push(&mut self, bytes: &[u8]) -> Result<Option<String>, LlamaError> {
        self.pending_bytes.extend_from_slice(bytes);
        self.flush_partial()
    }

    fn finish(&mut self) -> Result<Utf8FlushResult, LlamaError> {
        if self.pending_bytes.is_empty() {
            return Ok(Utf8FlushResult::default());
        }

        match std::str::from_utf8(&self.pending_bytes) {
            Ok(text) => {
                let text = text.to_owned();
                self.pending_bytes.clear();
                Ok(Utf8FlushResult { text: Some(text), dropped_incomplete_tail: false })
            }
            Err(error) if error.error_len().is_none() => {
                let valid_up_to = error.valid_up_to();
                let text = if valid_up_to == 0 {
                    None
                } else {
                    Some(
                        std::str::from_utf8(&self.pending_bytes[..valid_up_to])
                            .expect("valid UTF-8 prefix reported by std::str::from_utf8")
                            .to_owned(),
                    )
                };
                self.pending_bytes.clear();
                Ok(Utf8FlushResult { text, dropped_incomplete_tail: true })
            }
            Err(error) => Err(error.into()),
        }
    }

    fn clear(&mut self) {
        self.pending_bytes.clear();
    }

    fn has_pending(&self) -> bool {
        !self.pending_bytes.is_empty()
    }

    fn flush_partial(&mut self) -> Result<Option<String>, LlamaError> {
        if self.pending_bytes.is_empty() {
            return Ok(None);
        }

        match std::str::from_utf8(&self.pending_bytes) {
            Ok(text) => {
                let text = text.to_owned();
                self.pending_bytes.clear();
                Ok(Some(text))
            }
            Err(error) if error.error_len().is_none() => {
                let valid_up_to = error.valid_up_to();
                if valid_up_to == 0 {
                    return Ok(None);
                }

                let text = std::str::from_utf8(&self.pending_bytes[..valid_up_to])
                    .expect("valid UTF-8 prefix reported by std::str::from_utf8")
                    .to_owned();
                self.pending_bytes.drain(..valid_up_to);
                Ok(Some(text))
            }
            Err(error) => Err(error.into()),
        }
    }
}

enum GlobalCommand {
    CreateSession {
        grammar: Option<String>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        top_k: Option<i32>,
        min_p: Option<f32>,
        repetition_penalty: Option<f32>,
        presence_penalty: Option<f32>,
        ignore_eos: bool,
        logit_bias: Vec<LlamaLogitBias>,
        reply_tx: oneshot::Sender<Result<SessionId, LlamaRuntimeError>>,
    },
    CreateSessionFromSnapshot {
        grammar: Option<String>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        top_k: Option<i32>,
        min_p: Option<f32>,
        repetition_penalty: Option<f32>,
        presence_penalty: Option<f32>,
        ignore_eos: bool,
        logit_bias: Vec<LlamaLogitBias>,
        snapshot: LlamaSessionSnapshot,
        reply_tx: oneshot::Sender<Result<SessionId, LlamaRuntimeError>>,
    },
    AppendInput {
        session_id: SessionId,
        text_delta: String,
        reply_tx: oneshot::Sender<Result<(), LlamaRuntimeError>>,
    },
    GenerateStream {
        session_id: SessionId,
        max_new_tokens: usize,
        stream_tx: mpsc::Sender<StreamChunk>,
        reply_tx: oneshot::Sender<Result<(), LlamaRuntimeError>>,
    },
    EndSession {
        session_id: SessionId,
        reply_tx: oneshot::Sender<Result<(), LlamaRuntimeError>>,
    },
    SnapshotSession {
        session_id: SessionId,
        reply_tx: oneshot::Sender<Result<LlamaSessionSnapshot, LlamaRuntimeError>>,
    },
    Cancel {
        session_id: SessionId,
        reply_tx: oneshot::Sender<Result<(), LlamaRuntimeError>>,
    },
}

struct MasterWorkerState {
    global_rx: mpsc::Receiver<GlobalCommand>,
    worker_txs: Vec<mpsc::Sender<WorkerCommand>>,
    session_map: HashMap<SessionId, usize>,
    next_worker: usize,
    next_session_id: u64,
}

impl MasterWorkerState {
    async fn run(mut self) {
        while let Some(cmd) = self.global_rx.recv().await {
            match cmd {
                GlobalCommand::CreateSession {
                    grammar,
                    temperature,
                    top_p,
                    top_k,
                    min_p,
                    repetition_penalty,
                    presence_penalty,
                    ignore_eos,
                    logit_bias,
                    reply_tx,
                } => {
                    let session_id = self.next_session_id;
                    self.next_session_id += 1;
                    let worker_id = self.next_worker % self.worker_txs.len();
                    self.next_worker += 1;

                    let (ack_tx, ack_rx) = oneshot::channel();
                    if self.worker_txs[worker_id]
                        .send(WorkerCommand::CreateSession {
                            session_id,
                            grammar,
                            temperature,
                            top_p,
                            top_k,
                            min_p,
                            repetition_penalty,
                            presence_penalty,
                            ignore_eos,
                            logit_bias,
                            snapshot: None,
                            reply_tx: ack_tx,
                        })
                        .await
                        .is_err()
                    {
                        let _ = reply_tx.send(Err(LlamaRuntimeError::WorkerShutdown));
                        continue;
                    }
                    match ack_rx.await {
                        Ok(Ok(())) => {
                            self.session_map.insert(session_id, worker_id);
                            let _ = reply_tx.send(Ok(session_id));
                        }
                        Ok(Err(error)) => {
                            let _ = reply_tx.send(Err(error));
                        }
                        Err(_) => {
                            let _ = reply_tx.send(Err(LlamaRuntimeError::WorkerShutdown));
                        }
                    }
                }

                GlobalCommand::CreateSessionFromSnapshot {
                    grammar,
                    temperature,
                    top_p,
                    top_k,
                    min_p,
                    repetition_penalty,
                    presence_penalty,
                    ignore_eos,
                    logit_bias,
                    snapshot,
                    reply_tx,
                } => {
                    let session_id = self.next_session_id;
                    self.next_session_id += 1;
                    let worker_id = snapshot.worker_id % self.worker_txs.len();

                    let (ack_tx, ack_rx) = oneshot::channel();
                    if self.worker_txs[worker_id]
                        .send(WorkerCommand::CreateSession {
                            session_id,
                            grammar,
                            temperature,
                            top_p,
                            top_k,
                            min_p,
                            repetition_penalty,
                            presence_penalty,
                            ignore_eos,
                            logit_bias,
                            snapshot: Some(snapshot),
                            reply_tx: ack_tx,
                        })
                        .await
                        .is_err()
                    {
                        let _ = reply_tx.send(Err(LlamaRuntimeError::WorkerShutdown));
                        continue;
                    }
                    match ack_rx.await {
                        Ok(Ok(())) => {
                            self.session_map.insert(session_id, worker_id);
                            let _ = reply_tx.send(Ok(session_id));
                        }
                        Ok(Err(error)) => {
                            let _ = reply_tx.send(Err(error));
                        }
                        Err(_) => {
                            let _ = reply_tx.send(Err(LlamaRuntimeError::WorkerShutdown));
                        }
                    }
                }

                GlobalCommand::AppendInput { session_id, text_delta, reply_tx } => {
                    match self.session_map.get(&session_id) {
                        None => {
                            let _ = reply_tx
                                .send(Err(LlamaRuntimeError::SessionNotFound { session_id }));
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
                                let _ = reply_tx.send(Err(LlamaRuntimeError::WorkerShutdown));
                                continue;
                            }
                            match ack_rx.await {
                                Ok(result) => {
                                    let _ = reply_tx.send(result);
                                }
                                Err(_) => {
                                    let _ = reply_tx.send(Err(LlamaRuntimeError::WorkerShutdown));
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
                } => match self.session_map.get(&session_id) {
                    None => {
                        let _ =
                            reply_tx.send(Err(LlamaRuntimeError::SessionNotFound { session_id }));
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
                            let _ = reply_tx.send(Err(LlamaRuntimeError::WorkerShutdown));
                            continue;
                        }
                        match ack_rx.await {
                            Ok(result) => {
                                let _ = reply_tx.send(result);
                            }
                            Err(_) => {
                                let _ = reply_tx.send(Err(LlamaRuntimeError::WorkerShutdown));
                            }
                        }
                    }
                },

                GlobalCommand::EndSession { session_id, reply_tx } => {
                    match self.session_map.remove(&session_id) {
                        None => {
                            let _ = reply_tx
                                .send(Err(LlamaRuntimeError::SessionNotFound { session_id }));
                        }
                        Some(worker_id) => {
                            let (ack_tx, ack_rx) = oneshot::channel();
                            if self.worker_txs[worker_id]
                                .send(WorkerCommand::EndSession { session_id, reply_tx: ack_tx })
                                .await
                                .is_err()
                            {
                                let _ = reply_tx.send(Err(LlamaRuntimeError::WorkerShutdown));
                                continue;
                            }
                            match ack_rx.await {
                                Ok(result) => {
                                    let _ = reply_tx.send(result);
                                }
                                Err(_) => {
                                    let _ = reply_tx.send(Err(LlamaRuntimeError::WorkerShutdown));
                                }
                            }
                        }
                    }
                }

                GlobalCommand::SnapshotSession { session_id, reply_tx } => {
                    match self.session_map.get(&session_id) {
                        None => {
                            let _ = reply_tx
                                .send(Err(LlamaRuntimeError::SessionNotFound { session_id }));
                        }
                        Some(&worker_id) => {
                            let (ack_tx, ack_rx) = oneshot::channel();
                            if self.worker_txs[worker_id]
                                .send(WorkerCommand::SnapshotSession {
                                    session_id,
                                    reply_tx: ack_tx,
                                })
                                .await
                                .is_err()
                            {
                                let _ = reply_tx.send(Err(LlamaRuntimeError::WorkerShutdown));
                                continue;
                            }
                            match ack_rx.await {
                                Ok(Ok(mut snapshot)) => {
                                    snapshot.worker_id = worker_id;
                                    let _ = reply_tx.send(Ok(snapshot));
                                }
                                Ok(Err(error)) => {
                                    let _ = reply_tx.send(Err(error));
                                }
                                Err(_) => {
                                    let _ = reply_tx.send(Err(LlamaRuntimeError::WorkerShutdown));
                                }
                            }
                        }
                    }
                }

                GlobalCommand::Cancel { session_id, reply_tx } => {
                    match self.session_map.get(&session_id) {
                        None => {
                            let _ = reply_tx
                                .send(Err(LlamaRuntimeError::SessionNotFound { session_id }));
                        }
                        Some(&worker_id) => {
                            let (ack_tx, ack_rx) = oneshot::channel();
                            if self.worker_txs[worker_id]
                                .send(WorkerCommand::Cancel { session_id, reply_tx: ack_tx })
                                .await
                                .is_err()
                            {
                                let _ = reply_tx.send(Err(LlamaRuntimeError::WorkerShutdown));
                                continue;
                            }
                            match ack_rx.await {
                                Ok(result) => {
                                    let _ = reply_tx.send(result);
                                }
                                Err(_) => {
                                    let _ = reply_tx.send(Err(LlamaRuntimeError::WorkerShutdown));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub(super) enum WorkerCommand {
    CreateSession {
        session_id: SessionId,
        grammar: Option<String>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        top_k: Option<i32>,
        min_p: Option<f32>,
        repetition_penalty: Option<f32>,
        presence_penalty: Option<f32>,
        ignore_eos: bool,
        logit_bias: Vec<LlamaLogitBias>,
        snapshot: Option<LlamaSessionSnapshot>,
        reply_tx: oneshot::Sender<Result<(), LlamaRuntimeError>>,
    },
    AppendInput {
        session_id: SessionId,
        text_delta: String,
        reply_tx: oneshot::Sender<Result<(), LlamaRuntimeError>>,
    },
    GenerateStream {
        session_id: SessionId,
        max_new_tokens: usize,
        stream_tx: mpsc::Sender<StreamChunk>,
        reply_tx: oneshot::Sender<Result<(), LlamaRuntimeError>>,
    },
    EndSession {
        session_id: SessionId,
        reply_tx: oneshot::Sender<Result<(), LlamaRuntimeError>>,
    },
    SnapshotSession {
        session_id: SessionId,
        reply_tx: oneshot::Sender<Result<LlamaSessionSnapshot, LlamaRuntimeError>>,
    },
    Cancel {
        session_id: SessionId,
        reply_tx: oneshot::Sender<Result<(), LlamaRuntimeError>>,
    },
}

struct SessionState {
    seq_id: LlamaSeqId,
    n_past: i32,
    pending_tokens: Vec<LlamaToken>,
    pending_output: Utf8PieceBuffer,
    sampler: Option<crate::LlamaSampler>,
    stream_tx: Option<mpsc::Sender<StreamChunk>>,
    remaining_tokens: usize,
    last_token: Option<LlamaToken>,
    cancelled: bool,
}

struct InferenceWorkerState {
    #[allow(dead_code)]
    worker_id: usize,
    model: Arc<LlamaModel>,
    ctx: LlamaContext,
    sessions: HashMap<SessionId, SessionState>,
    next_seq_id: LlamaSeqId,
    free_seq_ids: Vec<LlamaSeqId>,
    max_seq_id_exclusive: LlamaSeqId,
    context_length: usize,
    kv_cache_can_shift: bool,
    window_drop_chunk: usize,
    cmd_rx: mpsc::Receiver<WorkerCommand>,
}

impl InferenceWorkerState {
    fn new(
        worker_id: usize,
        model: Arc<LlamaModel>,
        ctx: LlamaContext,
        cmd_rx: mpsc::Receiver<WorkerCommand>,
    ) -> Self {
        let context_length = ctx.n_ctx_seq() as usize;
        let max_seq_id_exclusive = i32::try_from(ctx.n_seq_max()).unwrap_or(i32::MAX);
        let kv_cache_can_shift = ctx.kv_cache_can_shift();
        let window_drop_chunk = (context_length / 4).max(1);

        Self {
            worker_id,
            model,
            ctx,
            sessions: HashMap::new(),
            next_seq_id: 0,
            free_seq_ids: Vec::new(),
            max_seq_id_exclusive,
            context_length,
            kv_cache_can_shift,
            window_drop_chunk,
            cmd_rx,
        }
    }

    fn fail_session_stream(session: &mut SessionState, message: impl Into<String>) {
        if let Some(tx) = session.stream_tx.take() {
            let _ = tx.blocking_send(StreamChunk::Error(message.into()));
        }
        session.pending_output.clear();
        session.remaining_tokens = 0;
        session.last_token = None;
    }

    fn build_stop_info(
        model: &LlamaModel,
        token: Option<LlamaToken>,
        finish_reason: &str,
    ) -> LlamaStopInfo {
        let (stop_token_text, stop_token_kind) = match token {
            Some(token) => {
                let token_text =
                    model.token_to_piece(token, true).ok().filter(|text| !text.is_empty());
                let token_kind = model.token_stop_kind(token).map(str::to_owned);
                (token_text, token_kind)
            }
            None => (None, None),
        };

        LlamaStopInfo {
            finish_reason: finish_reason.to_owned(),
            stop_token_id: token,
            stop_token_text,
            stop_token_kind,
        }
    }

    fn finish_session_stream(
        session: &mut SessionState,
        final_text: Option<String>,
        stop: Option<LlamaStopInfo>,
    ) -> Result<(), mpsc::error::SendError<StreamChunk>> {
        session.last_token = None;
        session.remaining_tokens = 0;

        let Some(tx) = session.stream_tx.take() else {
            session.pending_output.clear();
            return Ok(());
        };

        if let Some(text) = final_text
            && !text.is_empty()
        {
            tx.blocking_send(StreamChunk::Token(text))?;
        }

        if let Some(stop) = stop {
            tx.blocking_send(StreamChunk::Stop(stop))?;
        }

        session.pending_output.clear();
        tx.blocking_send(StreamChunk::Done)
    }

    fn describe_stream_error(&self, error: &LlamaError, batch_tokens: usize) -> String {
        match error {
            LlamaError::DecodeFailed(1) => {
                let active_sessions =
                    self.sessions.values().filter(|session| session.stream_tx.is_some()).count();
                format!(
                    "llama decode could not find a KV slot for the current batch (context_length={}, batch_tokens={}, active_sessions={}); this usually means the loaded context is too small or multiple sessions have exhausted the KV cache",
                    self.context_length, batch_tokens, active_sessions
                )
            }
            _ => error.to_string(),
        }
    }

    fn ensure_window_capacity(
        ctx: &mut LlamaContext,
        can_shift: bool,
        context_length: usize,
        window_drop_chunk: usize,
        session: &mut SessionState,
        needed_tokens: usize,
    ) -> Result<(), String> {
        if context_length == 0 || needed_tokens == 0 {
            return Ok(());
        }

        if needed_tokens > context_length {
            return Err(format!(
                "requested token chunk ({needed_tokens}) exceeds context length ({context_length})"
            ));
        }

        let n_past = session.n_past.max(0) as usize;
        if n_past + needed_tokens <= context_length {
            return Ok(());
        }

        let overflow = n_past + needed_tokens - context_length;

        if !can_shift {
            warn!(
                seq_id = session.seq_id,
                context_length, "KV cache shift unsupported; clearing session cache to continue"
            );
            let _ = ctx.kv_cache_seq_rm(session.seq_id, 0, i32::MAX);
            session.n_past = 0;
            session.last_token = None;
            return Ok(());
        }

        let mut drop = overflow.max(window_drop_chunk).min(n_past);
        if n_past.saturating_sub(drop) + needed_tokens > context_length {
            drop = n_past + needed_tokens - context_length;
        }
        if drop == 0 {
            return Ok(());
        }

        let drop_i32 = i32::try_from(drop).map_err(|_| {
            format!("window shift overflow: drop count {drop} does not fit into i32")
        })?;

        if !ctx.kv_cache_seq_rm(session.seq_id, 0, drop_i32) {
            return Err(format!(
                "failed to evict KV range [0, {drop_i32}) for seq_id={}",
                session.seq_id
            ));
        }
        ctx.kv_cache_seq_add(session.seq_id, drop_i32, -1, -drop_i32);
        session.n_past = session.n_past.saturating_sub(drop_i32);

        Ok(())
    }

    fn handle_command(&mut self, cmd: WorkerCommand) {
        match cmd {
            WorkerCommand::CreateSession {
                session_id,
                grammar,
                temperature,
                top_p,
                top_k,
                min_p,
                repetition_penalty,
                presence_penalty,
                ignore_eos,
                logit_bias,
                snapshot,
                reply_tx,
            } => {
                let seq_id = if let Some(reused) = self.free_seq_ids.pop() {
                    reused
                } else if self.next_seq_id < self.max_seq_id_exclusive {
                    let id = self.next_seq_id;
                    self.next_seq_id += 1;
                    id
                } else {
                    let _ = reply_tx.send(Err(LlamaRuntimeError::SessionCapacityExceeded {
                        max_sessions: self.max_seq_id_exclusive.max(0) as usize,
                    }));
                    return;
                };

                let sampler = self.model.new_sampler_with_options(
                    grammar.as_deref(),
                    temperature,
                    top_p,
                    top_k,
                    min_p,
                    repetition_penalty,
                    presence_penalty,
                    ignore_eos,
                    &logit_bias,
                );
                let mut state = SessionState {
                    seq_id,
                    n_past: 0,
                    pending_tokens: Vec::new(),
                    pending_output: Utf8PieceBuffer::default(),
                    sampler: Some(sampler),
                    stream_tx: None,
                    remaining_tokens: 0,
                    last_token: None,
                    cancelled: false,
                };

                if let Some(snapshot) = snapshot {
                    if let Err(source) =
                        self.ctx.state_seq_set_data(snapshot.state.as_ref(), seq_id)
                    {
                        self.free_seq_ids.push(seq_id);
                        let _ = reply_tx.send(Err(LlamaRuntimeError::RestoreState { source }));
                        return;
                    }
                    state.n_past = snapshot.n_past;
                }

                self.sessions.insert(session_id, state);
                let _ = reply_tx.send(Ok(()));
            }

            WorkerCommand::AppendInput { session_id, text_delta, reply_tx } => {
                match self.sessions.get_mut(&session_id) {
                    None => {
                        let _ =
                            reply_tx.send(Err(LlamaRuntimeError::SessionNotFound { session_id }));
                    }
                    Some(session) => {
                        let result = self
                            .model
                            .tokenize(&text_delta, false, true)
                            .map(|tokens| {
                                session.pending_tokens.extend(tokens);
                            })
                            .map_err(|source| LlamaRuntimeError::TokenizeFailed { source });
                        let _ = reply_tx.send(result);
                    }
                }
            }

            WorkerCommand::GenerateStream { session_id, max_new_tokens, stream_tx, reply_tx } => {
                match self.sessions.get_mut(&session_id) {
                    None => {
                        let _ =
                            reply_tx.send(Err(LlamaRuntimeError::SessionNotFound { session_id }));
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
                        let _ =
                            reply_tx.send(Err(LlamaRuntimeError::SessionNotFound { session_id }));
                    }
                    Some(session) => {
                        self.ctx.kv_cache_seq_rm(session.seq_id, 0, i32::MAX);
                        self.free_seq_ids.push(session.seq_id);
                        let _ = reply_tx.send(Ok(()));
                    }
                }
            }

            WorkerCommand::SnapshotSession { session_id, reply_tx } => match self
                .sessions
                .get(&session_id)
            {
                None => {
                    let _ = reply_tx.send(Err(LlamaRuntimeError::SessionNotFound { session_id }));
                }
                Some(session)
                    if !session.pending_tokens.is_empty()
                        || session.pending_output.has_pending()
                        || session.stream_tx.is_some()
                        || session.remaining_tokens > 0
                        || session.last_token.is_some()
                        || session.cancelled =>
                {
                    let _ = reply_tx.send(Err(LlamaRuntimeError::SessionNotIdle { session_id }));
                }
                Some(session) => {
                    let state_size = self.ctx.state_seq_get_size(session.seq_id);
                    let mut state = vec![0u8; state_size];
                    match self.ctx.state_seq_get_data(&mut state, session.seq_id) {
                        Ok(written) => {
                            state.truncate(written);
                            let _ = reply_tx.send(Ok(LlamaSessionSnapshot {
                                worker_id: self.worker_id,
                                n_past: session.n_past,
                                state: Arc::from(state),
                            }));
                        }
                        Err(source) => {
                            let _ = reply_tx.send(Err(LlamaRuntimeError::SnapshotState { source }));
                        }
                    }
                }
            },

            WorkerCommand::Cancel { session_id, reply_tx } => {
                match self.sessions.get_mut(&session_id) {
                    None => {
                        let _ =
                            reply_tx.send(Err(LlamaRuntimeError::SessionNotFound { session_id }));
                    }
                    Some(session) => {
                        session.cancelled = true;
                        let _ = reply_tx.send(Ok(()));
                    }
                }
            }
        }
    }

    fn has_work(&self) -> bool {
        self.sessions.values().any(|session| {
            !session.cancelled
                && session.stream_tx.is_some()
                && session.remaining_tokens > 0
                && (!session.pending_tokens.is_empty() || session.last_token.is_some())
        })
    }

    fn run_inference_step(&mut self) {
        let batch_capacity = self.ctx.n_batch() as usize;
        let mut batch = LlamaBatch::new(batch_capacity);
        let context_length = self.context_length;
        let kv_cache_can_shift = self.kv_cache_can_shift;
        let window_drop_chunk = self.window_drop_chunk;
        let mut logit_owners: Vec<(SessionId, i32)> = Vec::new();
        let mut prefill_counts: HashMap<SessionId, usize> = HashMap::new();
        let mut gen_sessions: Vec<SessionId> = Vec::new();

        let session_ids: Vec<SessionId> = self.sessions.keys().copied().collect();

        for &session_id in &session_ids {
            let session = self.sessions.get_mut(&session_id).expect("session id from map keys");

            if session.cancelled {
                if Self::finish_session_stream(session, None, None).is_err() {
                    session.stream_tx = None;
                }
                continue;
            }

            if session.stream_tx.is_none() || session.remaining_tokens == 0 {
                continue;
            }

            if !session.pending_tokens.is_empty() {
                let pending_len = session.pending_tokens.len();
                let available = batch_capacity.saturating_sub(batch.n_tokens() as usize);
                if available == 0 {
                    continue;
                }

                let mut take_n = pending_len.min(available);
                if context_length > 0 {
                    take_n = take_n.min(context_length);
                }
                if take_n == 0 {
                    continue;
                }

                if let Err(error) = Self::ensure_window_capacity(
                    &mut self.ctx,
                    kv_cache_can_shift,
                    context_length,
                    window_drop_chunk,
                    session,
                    take_n,
                ) {
                    Self::fail_session_stream(session, error);
                    continue;
                }

                let finishes_prefill = take_n == pending_len;

                for index in 0..take_n {
                    let token = session.pending_tokens[index];
                    let is_last = finishes_prefill && index + 1 == take_n;
                    let batch_token_index = batch.n_tokens();
                    batch
                        .add(token, session.n_past + index as i32, &[session.seq_id], is_last)
                        .expect("batch capacity verified; add cannot fail");
                    if is_last {
                        logit_owners.push((session_id, batch_token_index));
                    }
                }
                prefill_counts.insert(session_id, take_n);
            } else if let Some(last_token) = session.last_token
                && (batch.n_tokens() as usize) < batch_capacity
            {
                if let Err(error) = Self::ensure_window_capacity(
                    &mut self.ctx,
                    kv_cache_can_shift,
                    context_length,
                    window_drop_chunk,
                    session,
                    1,
                ) {
                    Self::fail_session_stream(session, error);
                    continue;
                }

                let batch_token_index = batch.n_tokens();
                batch
                    .add(last_token, session.n_past, &[session.seq_id], true)
                    .expect("batch capacity verified; add cannot fail");
                logit_owners.push((session_id, batch_token_index));
                gen_sessions.push(session_id);
            }
        }

        if batch.n_tokens() == 0 {
            return;
        }

        if let Err(error) = self.ctx.decode(&mut batch) {
            let message = self.describe_stream_error(&error, batch.n_tokens() as usize);
            for session_id in session_ids {
                if let Some(session) = self.sessions.get_mut(&session_id)
                    && session.stream_tx.is_some()
                {
                    if let Some(tx) = session.stream_tx.take() {
                        let _ = tx.blocking_send(StreamChunk::Error(message.clone()));
                    }
                    session.pending_output.clear();
                    session.remaining_tokens = 0;
                    session.last_token = None;
                }
            }
            return;
        }

        for (session_id, count) in prefill_counts {
            if let Some(session) = self.sessions.get_mut(&session_id) {
                session.pending_tokens.drain(..count);
                session.n_past += i32::try_from(count).unwrap_or(i32::MAX);
            }
        }
        for session_id in gen_sessions {
            if let Some(session) = self.sessions.get_mut(&session_id) {
                session.n_past = session.n_past.saturating_add(1);
            }
        }

        for (session_id, batch_token_index) in logit_owners {
            let Some(session) = self.sessions.get_mut(&session_id) else {
                continue;
            };
            let Some(mut sampler) = session.sampler.take() else {
                Self::fail_session_stream(session, "llama sampler missing from session state");
                continue;
            };

            let token = sampler.sample(&mut self.ctx, batch_token_index);
            session.sampler = Some(sampler);

            if self.model.token_is_eog(token) || session.remaining_tokens == 0 {
                let flush = match session.pending_output.finish() {
                    Ok(flush) => flush,
                    Err(error) => {
                        Self::fail_session_stream(session, error.to_string());
                        continue;
                    }
                };
                if flush.dropped_incomplete_tail {
                    warn!(
                        session_id,
                        seq_id = session.seq_id,
                        "llama generation ended with an incomplete UTF-8 tail; dropping trailing bytes"
                    );
                }
                let stop = self
                    .model
                    .token_is_eog(token)
                    .then(|| Self::build_stop_info(&self.model, Some(token), "stop"));
                if Self::finish_session_stream(session, flush.text, stop).is_err() {
                    session.stream_tx = None;
                }
                continue;
            }

            match self.model.token_to_piece_bytes(token, true) {
                Ok(piece) => {
                    let text = match session.pending_output.push(&piece) {
                        Ok(text) => text,
                        Err(error) => {
                            Self::fail_session_stream(session, error.to_string());
                            continue;
                        }
                    };

                    if let Some(tx) = session.stream_tx.as_ref()
                        && let Some(text) = text
                        && tx.blocking_send(StreamChunk::Token(text)).is_err()
                    {
                        session.stream_tx = None;
                        session.pending_output.clear();
                        session.remaining_tokens = 0;
                        session.last_token = None;
                        continue;
                    }

                    session.last_token = Some(token);
                    session.remaining_tokens = session.remaining_tokens.saturating_sub(1);
                    if session.remaining_tokens == 0 {
                        let flush = match session.pending_output.finish() {
                            Ok(flush) => flush,
                            Err(error) => {
                                Self::fail_session_stream(session, error.to_string());
                                continue;
                            }
                        };
                        if flush.dropped_incomplete_tail {
                            warn!(
                                session_id,
                                seq_id = session.seq_id,
                                "llama generation stopped at the token budget with an incomplete UTF-8 tail; dropping trailing bytes"
                            );
                        }
                        let stop = Some(Self::build_stop_info(&self.model, None, "length"));
                        if Self::finish_session_stream(session, flush.text, stop).is_err() {
                            session.stream_tx = None;
                        }
                    }
                }
                Err(error) => {
                    if let Some(tx) = session.stream_tx.take() {
                        let _ = tx.blocking_send(StreamChunk::Error(error.to_string()));
                    }
                    session.pending_output.clear();
                    session.remaining_tokens = 0;
                    session.last_token = None;
                }
            }
        }
    }

    fn run(mut self) {
        loop {
            while let Ok(cmd) = self.cmd_rx.try_recv() {
                self.handle_command(cmd);
            }

            if self.has_work() {
                self.run_inference_step();
                continue;
            }

            match self.cmd_rx.blocking_recv() {
                Some(cmd) => self.handle_command(cmd),
                None => break,
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct LlamaRuntime {
    global_tx: mpsc::Sender<GlobalCommand>,
}

impl LlamaRuntime {
    pub fn start(
        num_workers: usize,
        model: Arc<LlamaModel>,
        ctx_params: LlamaContextParams,
    ) -> Result<Self, LlamaRuntimeError> {
        if num_workers == 0 {
            return Err(LlamaRuntimeError::InvalidWorkerCount { num_workers });
        }

        let mut worker_txs: Vec<mpsc::Sender<WorkerCommand>> = Vec::with_capacity(num_workers);

        for worker_id in 0..num_workers {
            let (cmd_tx, cmd_rx) = mpsc::channel::<WorkerCommand>(128);
            worker_txs.push(cmd_tx);

            let ctx = model
                .new_context(ctx_params.clone())
                .map_err(|source| LlamaRuntimeError::CreateContext { source })?;

            let worker_state =
                InferenceWorkerState::new(worker_id, Arc::clone(&model), ctx, cmd_rx);

            std::thread::Builder::new()
                .name(format!("llama-worker-{worker_id}"))
                .spawn(move || worker_state.run())
                .map_err(|source| LlamaRuntimeError::SpawnWorkerFailed { source })?;
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

    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn create_session(&self) -> Result<SessionId, LlamaRuntimeError> {
        self.create_session_with_options(None, None, None, None, None, None, None, false, vec![])
            .await
    }

    pub async fn create_session_with_gbnf(
        &self,
        gbnf: Option<String>,
    ) -> Result<SessionId, LlamaRuntimeError> {
        self.create_session_with_options(gbnf, None, None, None, None, None, None, false, vec![])
            .await
    }

    pub async fn create_session_with_options(
        &self,
        gbnf: Option<String>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        top_k: Option<i32>,
        min_p: Option<f32>,
        repetition_penalty: Option<f32>,
        presence_penalty: Option<f32>,
        ignore_eos: bool,
        logit_bias: Vec<LlamaLogitBias>,
    ) -> Result<SessionId, LlamaRuntimeError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::CreateSession {
                grammar: gbnf,
                temperature,
                top_p,
                top_k,
                min_p,
                repetition_penalty,
                presence_penalty,
                ignore_eos,
                logit_bias,
                reply_tx,
            })
            .await
            .map_err(|_| LlamaRuntimeError::WorkerShutdown)?;
        reply_rx.await.map_err(|_| LlamaRuntimeError::WorkerShutdown)?
    }

    pub async fn create_session_from_snapshot(
        &self,
        snapshot: LlamaSessionSnapshot,
        gbnf: Option<String>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        top_k: Option<i32>,
        min_p: Option<f32>,
        repetition_penalty: Option<f32>,
        presence_penalty: Option<f32>,
        ignore_eos: bool,
        logit_bias: Vec<LlamaLogitBias>,
    ) -> Result<SessionId, LlamaRuntimeError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::CreateSessionFromSnapshot {
                grammar: gbnf,
                temperature,
                top_p,
                top_k,
                min_p,
                repetition_penalty,
                presence_penalty,
                ignore_eos,
                logit_bias,
                snapshot,
                reply_tx,
            })
            .await
            .map_err(|_| LlamaRuntimeError::WorkerShutdown)?;
        reply_rx.await.map_err(|_| LlamaRuntimeError::WorkerShutdown)?
    }

    pub async fn append_input(
        &self,
        session_id: SessionId,
        text_delta: String,
    ) -> Result<(), LlamaRuntimeError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::AppendInput { session_id, text_delta, reply_tx })
            .await
            .map_err(|_| LlamaRuntimeError::WorkerShutdown)?;
        reply_rx.await.map_err(|_| LlamaRuntimeError::WorkerShutdown)?
    }

    pub async fn generate_stream(
        &self,
        session_id: SessionId,
        max_new_tokens: usize,
    ) -> Result<StreamHandle, LlamaRuntimeError> {
        let (stream_tx, stream_rx) = mpsc::channel::<StreamChunk>(64);
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::GenerateStream { session_id, max_new_tokens, stream_tx, reply_tx })
            .await
            .map_err(|_| LlamaRuntimeError::WorkerShutdown)?;
        reply_rx.await.map_err(|_| LlamaRuntimeError::WorkerShutdown)??;
        Ok(stream_rx)
    }

    pub async fn end_session(&self, session_id: SessionId) -> Result<(), LlamaRuntimeError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::EndSession { session_id, reply_tx })
            .await
            .map_err(|_| LlamaRuntimeError::WorkerShutdown)?;
        reply_rx.await.map_err(|_| LlamaRuntimeError::WorkerShutdown)?
    }

    pub async fn snapshot_session(
        &self,
        session_id: SessionId,
    ) -> Result<LlamaSessionSnapshot, LlamaRuntimeError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::SnapshotSession { session_id, reply_tx })
            .await
            .map_err(|_| LlamaRuntimeError::WorkerShutdown)?;
        reply_rx.await.map_err(|_| LlamaRuntimeError::WorkerShutdown)?
    }

    pub async fn cancel_generate(&self, session_id: SessionId) -> Result<(), LlamaRuntimeError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::Cancel { session_id, reply_tx })
            .await
            .map_err(|_| LlamaRuntimeError::WorkerShutdown)?;
        reply_rx.await.map_err(|_| LlamaRuntimeError::WorkerShutdown)?
    }
}

#[cfg(test)]
mod tests {
    use super::{Utf8FlushResult, Utf8PieceBuffer};

    #[test]
    fn utf8_piece_buffer_waits_for_multibyte_sequence_completion() {
        let mut buffer = Utf8PieceBuffer::default();

        assert_eq!(buffer.push(&[0xE4]).expect("first byte should buffer"), None);
        assert_eq!(
            buffer.push(&[0xB8, 0xAD]).expect("remaining bytes should flush"),
            Some("\u{4E2D}".to_owned())
        );
    }

    #[test]
    fn utf8_piece_buffer_emits_valid_prefix_and_keeps_partial_tail() {
        let mut buffer = Utf8PieceBuffer::default();

        assert_eq!(
            buffer.push(&[b'a', 0xE4]).expect("valid prefix should flush"),
            Some("a".to_owned())
        );
        assert_eq!(
            buffer.push(&[0xB8, 0xAD]).expect("tail should complete"),
            Some("\u{4E2D}".to_owned())
        );
    }

    #[test]
    fn utf8_piece_buffer_finish_drops_incomplete_tail() {
        let mut buffer = Utf8PieceBuffer::default();

        assert_eq!(
            buffer.push(&[b'a']).expect("ASCII should flush immediately"),
            Some("a".to_owned())
        );
        assert_eq!(buffer.push(&[0xE4]).expect("partial UTF-8 should buffer"), None);
        assert_eq!(
            buffer.finish().expect("finish should succeed"),
            Utf8FlushResult { text: None, dropped_incomplete_tail: true }
        );
    }
}
