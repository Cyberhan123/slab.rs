## Technical Plan (Tokio) — Multi-worker, single model, session KV reuse, streaming output, global queue, no session migration

### 1) Goals / Non-goals
**Goals**
- One loaded `LlamaModel` shared across workers.
- Multiple inference workers running in parallel, each with its own `LlamaContext`.
- Session-based KV reuse (multi-turn chat) without clearing the whole context.
- `generate` returns **streaming** output.
- A **global ingress queue** for all requests.
- **No session migration**: a session is pinned to one worker for its lifetime.

**Non-goals**
- Compatibility with the old “single-call generate(prompt)” API.
- KV migration between workers.
- Advanced policies (chunk flush heuristics, backpressure tuning) for now.

---

### 2) High-level architecture
**Single process, multi-thread / multi-task (Tokio runtime).**

Components:
1. **Global Ingress Queue** (Tokio `mpsc`): all API calls enqueue commands here.
2. **Master Worker** (single Tokio task): the only consumer of the global queue.
   - Chooses a worker for new sessions.
   - Maintains a fixed mapping: `session_id -> worker_id`.
   - Forwards commands to the right inference worker queue.
3. **Inference Workers (N)**: each worker owns:
   - `Arc<LlamaModel>` (shared weights)
   - one `LlamaContext` (exclusive)
   - its own session table and scheduler (continuous batching within that context)
   - streaming token output directly to the client’s stream channel

Rationale:
- Master worker provides the “global queue” requirement and enforces session pinning.
- Each inference worker can run continuously without locks on hot paths.

---

### 3) Public API (session-oriented)
- `create_session() -> SessionId`
- `append_input(session_id, text_delta)`
- `generate_stream(session_id, max_new_tokens) -> StreamHandle`
- `end_session(session_id)`
- (recommended) `cancel_generate(session_id)` to stop current streaming generation but keep KV

**Important contract:** `append_input` is **delta input** (new user message), not the full conversation history.

---

### 4) Command protocol (Tokio channels)
**Global queue commands** (handled by Master):
- `CreateSession`
- `AppendInput { session_id, text_delta }`
- `GenerateStream { session_id, max_new_tokens, stream_tx }`
- `EndSession { session_id }`
- `Cancel { session_id }` (optional)

**Acknowledgements:** use `oneshot` per command for success/error.  
**Streaming output:** separate per-request stream channel (e.g., Tokio `mpsc`) returned to caller as a `StreamHandle`.

Master responsibilities per command:
- `CreateSession`: pick worker → forward → receive new session_id → store mapping.
- Others: lookup mapping → forward to worker → await ack.
- `EndSession`: after worker confirms release, master removes mapping.

---

### 5) Worker ownership model (key safety point)
- Each inference worker is the exclusive owner of its `LlamaContext`.
- No other task/thread calls `decode` on that context.
- `LlamaContext` being `Send` is helpful for initialization, but **the runtime rule remains: single-owner execution**.

---

### 6) Session state (inside an inference worker)
Each session stores at minimum:
- `seq_id` (per-context sequence id)
- `n_past` (current position counter for that sequence)
- `pending_tokens` (tokenized delta input to prefill)
- `sampler` (per-session sampler to avoid cross-session contamination)
- streaming state: active stream sender, generation budget, cancel flag, etc.

---

### 7) KV cache strategy (hard requirement)
- Never call `kv_cache_clear()` in normal operation.
- Use **sequence-scoped** cleanup:
  - On `end_session`: `kv_cache_seq_rm(seq_id, 0, i32::MAX)` to remove only that session KV.
- Maintain positions strictly:
  - Every token written to the context uses `pos = n_past`, then `n_past += 1` (per session).

This is what enables multi-turn reuse and multi-session coexistence within one context.

---

### 8) Scheduling + batching model (within a worker)
Each inference worker runs a loop with **continuous batching** across sessions in its context:
- Prefill pending input for multiple sessions in one `LlamaBatch`.
- Decode one-step generation across multiple running sessions in one `LlamaBatch`.
- Use `logit=true` selectively and maintain a `logit_owner` list so that:
  - `sampler.sample(ctx, idx)` corresponds to the correct session’s logits (by the ordering of `logit=true` entries in the last decoded batch).

Fairness policy (initial):
- Round-robin across running sessions; each tick advances each session by at most one token (or a small bounded number).

---

### 9) Streaming generate semantics
`generate_stream(session, max_new_tokens)`:
- Does **not** run generation to completion inline.
- It **registers** a running generation for that session (budget + stream sender).
- The worker loop emits `Chunk` messages as tokens are produced.
- Completion conditions:
  - budget exhausted
  - EOG token
  - cancel
  - error
- On completion: send `Done` (best-effort) and close the stream.

**Backpressure policy (high level for now):**
- Do not block the inference loop waiting on a slow consumer.
- If the stream channel cannot accept messages, terminate the stream/generation for that session (best-effort notification + close).

---

### 10) Multi-worker scaling model
- Model weights are shared (`Arc<LlamaModel>`).
- Each worker has its own context and its own internal batching.
- Master worker distributes **new** sessions across workers (RR or least-loaded).
- No session migration: all subsequent commands for a session always go to the same worker.

---

### 11) Implementation order (practical)
1. Implement one inference worker + master worker + global queue + session API.
2. Implement session KV reuse (`seq_id`, `n_past`, `kv_cache_seq_rm` on end).
3. Implement streaming generate (register + emit from worker loop).
4. Add continuous batching inside the worker.
5. Scale to N workers; add master session distribution and mapping.

---

### 12) Key takeaways
- **Global queue** exists and is consumed by a **Master Worker**.
- **Session pinning** is enforced via `session_id -> worker_id` mapping.
- **KV reuse** is achieved via per-session `seq_id` and position tracking; cleanup uses `kv_cache_seq_rm`, not `kv_cache_clear`.
- **Streaming** is produced by inference workers directly; master does not relay token chunks.
- **Parallelism** comes from multiple contexts (workers) + batching within each context.