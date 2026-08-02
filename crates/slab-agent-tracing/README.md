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
