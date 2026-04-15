use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tracing::warn;

use crate::{
    ChatMessage, LlamaBatch, LlamaContext, LlamaContextParams, LlamaError, LlamaModel, LlamaSeqId,
    LlamaToken,
};

pub type SessionId = u64;

#[derive(Debug, Clone)]
pub struct LlamaSessionSnapshot {
    pub worker_id: usize,
    pub n_past: i32,
    pub state: Arc<[u8]>,
}

/// Minimal GBNF grammar that constrains output to a valid JSON value.
pub const GRAMMAR_JSON: &str = r#"root   ::= value
value  ::= object | array | string | number | "true" | "false" | "null"
object ::=
    "{" ws (
                        string ":" ws value
        ("," ws string ":" ws value)*
    )? "}" ws
array ::=
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

/// GBNF grammar for tool-call envelope: {"tool":"<name>","arguments":{...}}.
pub const GRAMMAR_TOOL_CALL: &str = r#"root      ::= "{" ws "\"tool\"" ws ":" ws string ws "," ws "\"arguments\"" ws ":" ws object ws "}"
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

pub fn resolve_grammar(
    grammar: Option<&str>,
    grammar_json: bool,
    grammar_tool_call: bool,
) -> Option<String> {
    if let Some(grammar) = grammar
        && !grammar.is_empty()
    {
        return Some(grammar.to_owned());
    }
    if grammar_json {
        return Some(GRAMMAR_JSON.to_owned());
    }
    if grammar_tool_call {
        return Some(GRAMMAR_TOOL_CALL.to_owned());
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlamaLoadConfig {
    pub model_path: PathBuf,
    pub num_workers: usize,
    pub context_length: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_template: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlamaInferenceParams {
    pub max_tokens: usize,
    pub session_key: Option<String>,
    pub apply_chat_template: bool,
    pub chat_messages: Vec<ChatMessage>,
    pub grammar: Option<String>,
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
    Done,
    Error(String),
}

pub type StreamHandle = mpsc::Receiver<StreamChunk>;

enum GlobalCommand {
    CreateSession {
        grammar: Option<String>,
        reply_tx: oneshot::Sender<Result<SessionId, LlamaRuntimeError>>,
    },
    CreateSessionFromSnapshot {
        grammar: Option<String>,
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
                GlobalCommand::CreateSession { grammar, reply_tx } => {
                    let session_id = self.next_session_id;
                    self.next_session_id += 1;
                    let worker_id = self.next_worker % self.worker_txs.len();
                    self.next_worker += 1;

                    let (ack_tx, ack_rx) = oneshot::channel();
                    if self.worker_txs[worker_id]
                        .send(WorkerCommand::CreateSession {
                            session_id,
                            grammar,
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

                GlobalCommand::CreateSessionFromSnapshot { grammar, snapshot, reply_tx } => {
                    let session_id = self.next_session_id;
                    self.next_session_id += 1;
                    let worker_id = snapshot.worker_id % self.worker_txs.len();

                    let (ack_tx, ack_rx) = oneshot::channel();
                    if self.worker_txs[worker_id]
                        .send(WorkerCommand::CreateSession {
                            session_id,
                            grammar,
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
        let context_length = ctx.n_ctx() as usize;
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
        session.remaining_tokens = 0;
        session.last_token = None;
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
            WorkerCommand::CreateSession { session_id, grammar, snapshot, reply_tx } => {
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

                let sampler = self.model.new_sampler_with_grammar(grammar.as_deref());
                let mut state = SessionState {
                    seq_id,
                    n_past: 0,
                    pending_tokens: Vec::new(),
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

            if token == self.model.token_eos() || session.remaining_tokens == 0 {
                session.last_token = None;
                session.remaining_tokens = 0;
                if let Some(tx) = session.stream_tx.take() {
                    let _ = tx.blocking_send(StreamChunk::Done);
                }
                continue;
            }

            match self.model.token_to_piece(token, true) {
                Ok(piece) => {
                    if let Some(tx) = session.stream_tx.as_ref() {
                        match tx.blocking_send(StreamChunk::Token(piece)) {
                            Ok(()) => {
                                session.last_token = Some(token);
                                session.remaining_tokens =
                                    session.remaining_tokens.saturating_sub(1);
                            }
                            Err(_) => {
                                session.stream_tx = None;
                                session.remaining_tokens = 0;
                                session.last_token = None;
                            }
                        }
                    }
                }
                Err(error) => {
                    if let Some(tx) = session.stream_tx.take() {
                        let _ = tx.blocking_send(StreamChunk::Error(error.to_string()));
                    }
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
        self.create_session_with_grammar(None).await
    }

    pub async fn create_session_with_grammar(
        &self,
        grammar: Option<String>,
    ) -> Result<SessionId, LlamaRuntimeError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::CreateSession { grammar, reply_tx })
            .await
            .map_err(|_| LlamaRuntimeError::WorkerShutdown)?;
        reply_rx.await.map_err(|_| LlamaRuntimeError::WorkerShutdown)?
    }

    pub async fn create_session_from_snapshot(
        &self,
        snapshot: LlamaSessionSnapshot,
        grammar: Option<String>,
    ) -> Result<SessionId, LlamaRuntimeError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::CreateSessionFromSnapshot { grammar, snapshot, reply_tx })
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
