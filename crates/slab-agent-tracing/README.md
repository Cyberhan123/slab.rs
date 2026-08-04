# slab-agent-tracing
	Session-scoped agent trace logging for Slab.

## Role

`slab-agent-tracing` provides the trace sink API used to record structured
agent lifecycle events, the trace bundle directory used for L3 semantic replay,
and (as of Slice 10) the offline conversation reducer. It includes:

- Trace context and event payload types (`AgentTraceContext`, `AgentTraceEvent`).
- A no-op sink for disabled tracing.
- A file sink for per-session JSONL logs that also emits a session-telemetry
  event through `tracing` (target `slab_otel::session`).
- Helpers for stable session log names and paths.
- A typed trace bundle: `bundle` (directory + manifest) and `writer`
  (`trace.jsonl` + payload-before-event invariant), with the typed
  `RawTraceEvent` taxonomy in `event`.
- An offline conversation `reducer` (Slice 10) that folds a bundle's many
  inferences into the linear conversation the model was shown.

It does not decide when tracing is enabled, where application logs live, or how
telemetry is exported. Host configuration and lifecycle wiring belong in
`crates/slab-app-core`, `crates/slab-config`, and `crates/slab-otel`.

## L1 (rollout) vs L3 (reducer) — the two diagnostic layers

slab has two complementary diagnostic layers, kept deliberately distinct:

- **L1 — rollout** (`slab-agent-rollout`): records **what happened**. An
  append-only JSONL true source of every turn item, conversation delta, and
  compaction snapshot. Authoritative for session persistence and history replay.
- **L3 — trace bundle + reducer** (this crate): reconstructs **what the model
  saw**. The bundle stores the raw LLM request/response payloads the model was
  called with; the reducer folds the many inferences into the single linear
  conversation the model was shown. This is the semantic layer that answers
  "given these raw calls, what did the model actually receive?".

The L1 rollout is the source of truth for *events*; the L3 reducer is the
interpreter for *model-visible context*. They are independent: a rollout file
can be replayed without a trace bundle, and a trace bundle can be reduced
without the rollout.

## Decoupling from slab-otel (Slice 8)

This crate no longer depends on `slab-otel`. The file sink emits its
session-telemetry event directly via `tracing::info!` with the same target
(`slab_otel::session`) and field names/order as
`slab_otel::SessionTelemetry::emit_event`, so downstream OTel/subscriber
filters see a byte-identical wire format. `slab-otel` remains a dependency of
`slab-agent` and `slab-app-core` for gen_ai metrics (target `slab_otel::gen_ai`),
which is unrelated to this crate.

## Trace bundle layout (Slice 9)

When the main process enables a trace bundle, events are grouped per root
thread under `<logs_dir>/agent_trace/`:

```text
trace-<trace_uuid>-<root_thread_id>/
    manifest.json     # written ONCE at creation (trace_id, root_thread_id, created_at, rollout_path, format_version)
    trace.jsonl       # append-only typed event stream (RawTraceEvent)
    payloads/*.json   # large bodies referenced by events (request/response/tool)
    state.json        # reducer cache (Slice 10); written after a successful reduction
```

The `TraceWriter` enforces a **payload-before-event invariant**: a payload file
under `payloads/` is always durable (written + flushed) before the event
referencing it is appended to `trace.jsonl`. A reducer replaying the bundle can
therefore assume any referenced payload exists.

The legacy ~50 `record_json` / `record_json_from_context` call sites keep their
free-form `source`/`event`/`payload` shape; they are carried through the typed
taxonomy's `Other` catch-all variant and are NOT migrated by this work.

## Live bundle sink (Slice 0 hot-path wiring)

[`BundleAgentTraceSink`] is the production sink `slab-app-core` bootstrap assembles
when `agent.debug` is on. It COMPOSES a [`FileAgentTraceSink`] (so the legacy
per-session JSONL and the byte-identical `slab_otel::session` telemetry wire stay
alive) and ADDITIONALLY records every **main-process `slab-agent`** event into a
per-root-thread bundle.

### What reaches the bundle vs. the legacy JSONL only (scope boundary)

- **Into the bundle**: main-process `slab-agent` events flowing through the
  `trace_sink` (`record_json` in `crates/slab-agent/**` — turn lifecycle, LLM
  request/response, tool calls, compaction, thread lifecycle). The sink is
  assembled by `slab-app-core` bootstrap and shared with every spawned thread.
- **Legacy JSONL ONLY** (bypasses the bundle, this slice): `record_json_from_context`
  callers in `slab-app-core` (`domain/services/chat/local.rs`,
  `infra/agent/adapter.rs`) and `slab-runtime` (cross-process). Those resolve a
  shared `FileAgentTraceSink` from the per-`trace_dir` registry and write the
  per-session JSONL + telemetry wire, but do NOT route into the per-root-thread
  bundle. This is a deliberate Slice 0 trade-off: coordinating cross-process /
  adapter writes into the same bundle's `payloads/` is deferred (see
  "Cross-process write split" below). The consequence: a bundle covers the
  `slab-agent` orchestration view; the runtime/adapter LLM-request payloads are
  in the legacy JSONL until a later slice bridges them.

- **Lazy bundle**: the bundle (and its once-written manifest) is created on the
  sink's first sight of a root thread id, then cached. The bundle directory is
  deterministic — `bundle_dir_for_root_thread(trace_dir, root_thread_id)` — so
  the rollout `SessionMeta.trace_path` (set on the ROOT thread by
  `slab-app-core::build_session_meta`) and the sink's output dir are the SAME
  path. `trace_id` is derived from `root_thread_id` (no random uuid) precisely
  so the path is reproducible by both sides.
- **Free-form → typed bridge**: high-frequency event names map to the typed
  `RawTraceEventPayload` variants (`agent_llm_request` → `InferenceStarted`,
  `llm_response_normalized` → `InferenceCompleted`, `tool_call_started` →
  `ToolCallStarted`, `tool_call_output`/`tool_calls_completed` →
  `ToolCallCompleted`, `turn_started`/`turn_completed` → `TurnStarted`/
  `TurnCompleted`, `context_compaction_completed` → `ContextCompacted`);
  everything else falls through to the `Other` catch-all so an event is NEVER
  dropped. Inference events use EXACT name matching (`agent_llm_request`,
  `llm_response_normalized`, `chat_response_normalized`) — a substring `request`
  match would misclassify the real `structured_output_requested` event (emitted
  every turn when structured output is configured) as a phantom inference
  request; tool/turn/compaction use case-insensitive substring (no real-name
  collisions there). Marker variants carry no payload (the data is on the event
  envelope + the rollout true source).
- **Per-record writer**: a fresh `TraceWriter` is opened per record so each
  event is stamped with its own `thread_id`/`turn_index`/`parent_span_id` (a
  root thread and a child thread share one bundle but carry distinct stamps).
- **Diagnostic-only failures**: any bundle/payload/append error is logged at
  `warn!` and the event is simply not recorded to the bundle — agent execution
  always continues.

`slab-agent` stamps `root_thread_id` onto its trace context (root → its own id;
child → its TRUE root, resolved by walking the persisted parent chain up to the
ancestor with no parent, so a depth-N spawn chain — nested `DelegateSubagentTool`
delegation, bounded by `max_depth` — ALL groups into the SAME root bundle). If the
chain cannot be walked (a parent snapshot not yet persisted — a diagnostic race),
the child falls back to its nearest ancestor so the event is still grouped
deterministically. `slab-runtime` (cross-process) is UNCHANGED this slice: it keeps
writing the legacy per-session JSONL via `record_json_from_context`.

## Conversation reducer (Slice 10) — OFFLINE

The `reducer` module is an **offline diagnostic**: it is never wired into the
agent hot path. It reads a finished (or in-progress) trace bundle and folds the
many inferences into the linear conversation the model was shown. Three folding
modes, processed in event order:

1. **AppendOnly** — a request carrying a `previous_response_id` (resolved to a
   response in the CURRENT lineage) appends only the delta VERBATIM; the prefix
   is reused. Delta messages are never deduped: a legitimately repeated delta
   (the same user text twice, or an identical tool result twice) is NEW input
   the model received and must be kept. The chain is walked implicitly because
   events are processed in order.
2. **FullSnapshot** — a request with no `previous_response_id` (or an unresolved
   one — including a response id replaced out of the lineage by an earlier
   snapshot) becomes the conversation verbatim, reusing item ids where
   fingerprints match (no duplication) and dropping items no longer present. The
   replace INVALIDATES the response_index, so a later AppendOnly request
   referencing a pre-snapshot response id cannot resolve it and falls back to
   FullSnapshot.
3. **Post-compaction snapshot** — after a `ContextCompacted` event, the next
   full request carries the post-compaction history; the FullSnapshot replace
   drops the pre-compaction items compaction replaced.

Deduplication happens ONLY at the explicit prefix-reuse point (the FullSnapshot
replace); the delta / new-message append paths always append verbatim. Payload
fields (`previous_response_id`, response `id`, message lists) are parsed
best-effort from each payload's JSON; a missing field falls back to FullSnapshot
and never panics. After a successful reduction the conversation is cached to the
bundle's `state.json` (`reduce_conversation_cached`); a re-run reuses the cache
only when it covers EXACTLY every current `trace.jsonl` line (a truncated/shrunk
trace forces a re-derive, symmetric with the appended/grown case), otherwise
re-derives.

### Cross-process write split

`slab-runtime` is a separate process. Today only the **main process**
(`slab-app-core` / `slab-agent`) writes a trace bundle; `slab-runtime` continues
to write the simple per-session JSONL via `record_json_from_context` (it carries
an `AgentTraceContext` across the FFI boundary but does not write into a bundle).
Coordinating multi-process writes into one bundle's `payloads/` directory
(per-process ordinal prefixing, e.g. `payloads/<pid>-<ordinal>.json`, or file
locking) is a deferred follow-up.

## Two independent diagnostic switches (Slice 11)

`slab-app-core` bootstrap gates diagnostics on TWO independent switches:

- `agent.debug` — the trace sink + trace bundle (this crate). On alone → the
  user gets the trace bundle even with OTel export off.
- `telemetry.enabled` — OTel PROVIDER assembly + export (handled in the
  server/app/runtime init). On alone → OTel export runs without the trace bundle.

Both on = both. The trace-sink gate was decoupled from `telemetry.enabled` in
Slice 11 because this crate no longer depends on `slab-otel`; the OTel provider
gate is intentionally left untouched. The rollout ↔ trace coordination is wired
via `SessionMeta.trace_path` (set on the ROOT thread's session header so a
diagnostic can jump from the rollout file to the trace directory; child threads
correlate back via `root_thread_id`).

## Type

Rust library crate.

## Testing

Run focused tests with:

```sh
cargo test -p slab-agent-tracing
```

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
