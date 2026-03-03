use slab_llama::{LlamaContextParams, LlamaModel};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use super::worker::{InferenceWorkerState, WorkerCommand};
use super::{GGMLLlamaEngineError, SessionId, StreamChunk, StreamHandle};

// ── Master worker ─────────────────────────────────────────────────────────────

/// Commands sent by API callers to the global ingress queue (master worker).
enum GlobalCommand {
    CreateSession {
        reply_tx: oneshot::Sender<Result<SessionId, GGMLLlamaEngineError>>,
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
                        .send(WorkerCommand::CreateSession {
                            session_id,
                            reply_tx: ack_tx,
                        })
                        .await
                        .is_err()
                    {
                        let _ = reply_tx.send(Err(GGMLLlamaEngineError::WorkerShutdown));
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
                            let _ = reply_tx.send(Err(GGMLLlamaEngineError::WorkerShutdown));
                        }
                    }
                }

                GlobalCommand::AppendInput {
                    session_id,
                    text_delta,
                    reply_tx,
                } => match self.session_map.get(&session_id) {
                    None => {
                        let _ = reply_tx
                            .send(Err(GGMLLlamaEngineError::SessionNotFound { session_id }));
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
                            let _ = reply_tx.send(Err(GGMLLlamaEngineError::WorkerShutdown));
                            continue;
                        }
                        match ack_rx.await {
                            Ok(r) => {
                                let _ = reply_tx.send(r);
                            }
                            Err(_) => {
                                let _ = reply_tx.send(Err(GGMLLlamaEngineError::WorkerShutdown));
                            }
                        }
                    }
                },

                GlobalCommand::GenerateStream {
                    session_id,
                    max_new_tokens,
                    stream_tx,
                    reply_tx,
                } => match self.session_map.get(&session_id) {
                    None => {
                        let _ = reply_tx
                            .send(Err(GGMLLlamaEngineError::SessionNotFound { session_id }));
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
                            let _ = reply_tx.send(Err(GGMLLlamaEngineError::WorkerShutdown));
                            continue;
                        }
                        match ack_rx.await {
                            Ok(r) => {
                                let _ = reply_tx.send(r);
                            }
                            Err(_) => {
                                let _ = reply_tx.send(Err(GGMLLlamaEngineError::WorkerShutdown));
                            }
                        }
                    }
                },

                GlobalCommand::EndSession {
                    session_id,
                    reply_tx,
                } => match self.session_map.get(&session_id).copied() {
                    None => {
                        let _ = reply_tx
                            .send(Err(GGMLLlamaEngineError::SessionNotFound { session_id }));
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
                            let _ = reply_tx.send(Err(GGMLLlamaEngineError::WorkerShutdown));
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
                                let _ = reply_tx.send(Err(GGMLLlamaEngineError::WorkerShutdown));
                            }
                        }
                    }
                },

                GlobalCommand::Cancel {
                    session_id,
                    reply_tx,
                } => match self.session_map.get(&session_id) {
                    None => {
                        let _ = reply_tx
                            .send(Err(GGMLLlamaEngineError::SessionNotFound { session_id }));
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
                            let _ = reply_tx.send(Err(GGMLLlamaEngineError::WorkerShutdown));
                            continue;
                        }
                        match ack_rx.await {
                            Ok(r) => {
                                let _ = reply_tx.send(r);
                            }
                            Err(_) => {
                                let _ = reply_tx.send(Err(GGMLLlamaEngineError::WorkerShutdown));
                            }
                        }
                    }
                },
            }
        }
    }
}

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
#[derive(Clone, Debug)]
pub(super) struct LlamaInferenceEngine {
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
    /// `blocking_recv()` call. No explicit `shutdown()` call is required.
    ///
    /// # Panics
    /// Panics if called outside of a Tokio runtime.
    pub(super) fn start(
        num_workers: usize,
        model: Arc<LlamaModel>,
        ctx_params: LlamaContextParams,
    ) -> Result<Self, GGMLLlamaEngineError> {
        assert!(num_workers > 0, "num_workers must be > 0");

        let mut worker_txs: Vec<mpsc::Sender<WorkerCommand>> = Vec::with_capacity(num_workers);

        for worker_id in 0..num_workers {
            let (cmd_tx, cmd_rx) = mpsc::channel::<WorkerCommand>(128);
            worker_txs.push(cmd_tx);

            let ctx = model.new_context(ctx_params.clone()).map_err(|source| {
                GGMLLlamaEngineError::CreateContext {
                    source: source.into(),
                }
            })?;

            let worker_state =
                InferenceWorkerState::new(worker_id, Arc::clone(&model), ctx, cmd_rx);

            std::thread::Builder::new()
                .name(format!("llama-worker-{worker_id}"))
                .spawn(move || worker_state.run())
                .map_err(|source| GGMLLlamaEngineError::SpawnWorkerFailed { source })?;
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
    pub(super) async fn create_session(&self) -> Result<SessionId, GGMLLlamaEngineError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::CreateSession { reply_tx })
            .await
            .map_err(|_| GGMLLlamaEngineError::WorkerShutdown)?;
        reply_rx
            .await
            .map_err(|_| GGMLLlamaEngineError::WorkerShutdown)?
    }

    /// Append delta text to the session's input buffer.
    ///
    /// The text is tokenized and queued for prefilling. Call this before
    /// [`Self::generate_stream`] to populate the context with the new turn's
    /// prompt.
    pub(super) async fn append_input(
        &self,
        session_id: SessionId,
        text_delta: String,
    ) -> Result<(), GGMLLlamaEngineError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::AppendInput {
                session_id,
                text_delta,
                reply_tx,
            })
            .await
            .map_err(|_| GGMLLlamaEngineError::WorkerShutdown)?;
        reply_rx
            .await
            .map_err(|_| GGMLLlamaEngineError::WorkerShutdown)?
    }

    /// Start streaming generation for a session.
    ///
    /// Returns a [`StreamHandle`] that receives [`StreamChunk`] items as the
    /// inference worker produces them. The stream is closed by the worker
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
    /// the caller drops it). Use [`Self::cancel_generate`] first if you need
    /// an explicit `Done` on the previous stream.
    pub(super) async fn generate_stream(
        &self,
        session_id: SessionId,
        max_new_tokens: usize,
    ) -> Result<StreamHandle, GGMLLlamaEngineError> {
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
            .map_err(|_| GGMLLlamaEngineError::WorkerShutdown)?;
        reply_rx
            .await
            .map_err(|_| GGMLLlamaEngineError::WorkerShutdown)??;
        Ok(stream_rx)
    }

    /// End a session, releasing its KV-cache entries.
    ///
    /// Uses `kv_cache_seq_rm` internally so other sessions' KV data is
    /// unaffected.
    pub(super) async fn end_session(
        &self,
        session_id: SessionId,
    ) -> Result<(), GGMLLlamaEngineError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::EndSession {
                session_id,
                reply_tx,
            })
            .await
            .map_err(|_| GGMLLlamaEngineError::WorkerShutdown)?;
        reply_rx
            .await
            .map_err(|_| GGMLLlamaEngineError::WorkerShutdown)?
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
    /// to the caller. To continue a conversation from the exact emitted text,
    /// re-append that final text delta with [`Self::append_input`] before
    /// calling [`Self::generate_stream`] again.
    pub(super) async fn cancel_generate(
        &self,
        session_id: SessionId,
    ) -> Result<(), GGMLLlamaEngineError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.global_tx
            .send(GlobalCommand::Cancel {
                session_id,
                reply_tx,
            })
            .await
            .map_err(|_| GGMLLlamaEngineError::WorkerShutdown)?;
        reply_rx
            .await
            .map_err(|_| GGMLLlamaEngineError::WorkerShutdown)?
    }
}
