# slab-agent-rollout

Append-only JSONL event-source for slab agent sessions — the **L1 rollout true
source**. Each agent thread owns one file under
`<app_home>/sessions/<thread_id>.rollout.jsonl`.

## Why

slab's session persistence moved from SQLite three-table snapshots to an
event-sourced model where the rollout JSONL is the single source of truth and the
SQLite tables degrade into a rebuildable index. This crate owns the JSONL
read/write engine and the `RolloutStore` abstraction. It deliberately depends
**upward** on `slab-agent` (for the `TurnItem` / `EventMsg` payload types) and on
`slab-types` (for `ConversationMessage`); `slab-agent` never depends on it, so the
agent core stays pure.

## Part 1 status — finished

The rollout JSONL is the true source; the `RolloutBackedAgentStore` adapter (in
`slab-app-core`) is the **only** `AgentStorePort` impl, wired in by the bootstrap.
The integration pieces that consume this crate live in `slab-app-core`:

- **Adapter** (`infra/agent/rollout_store.rs`): metadata + tool-call audit stay on
  SQLite; conversation writes (`MessageAppend` / `TurnState` / `TurnItem`) and all
  reads route through the rollout file. The read gate flips to rollout-first once a
  thread's `rollout_session_index.backfill_status == "completed"` (a new thread is
  stamped at creation; a legacy thread is flipped by the startup backfill).
- **Observer** (`infra/agent/rollout_persistence.rs`): one background task per
  thread bridges the harness `EventMsg` stream into rollout lines
  (`ItemCompleted` → `TurnItem`, `ContextCompacted` → `Compacted`, filtered
  lifecycle → `EventMsg`).
- **Migration + backfill** (`migrations/20260802000000_introduce_rollout_index.sql`
  + `infra/agent/rollout_backfill.rs`): the index tables are created at boot and
  legacy SQL rows are copied into rollout JSONL once, asynchronously.
- **Harness operations** (`domain/services/agent/harness.rs`) act on the rollout
  directly via `AgentCore::rollout()`:
  - **compact** — `truncate_from_turn(0)` (keeps `SessionMeta`) + one `Compacted`
    line carrying the compacted set (`status = "manual"`); the line becomes the new
    `read_messages` baseline. Compacted messages are NOT re-inserted as
    `MessageAppend` lines (that would duplicate them on read).
  - **rollback** — a single atomic `truncate_from_turn(to_turn + 1)` collapses the
    old three-way per-table delete into one file truncation.
  - **fork** — `control.fork_thread` (pure slab-agent) creates the child metadata,
    then the harness rebuilds the child rollout wholesale from the parent's lines
    (preserving the child `SessionMeta` with `parent_id`) so turn attribution
    survives — the per-row adapter copy would batch all `TurnContext` before all
    `TurnItem` lines and break `read_turn_items`'s running-turn heuristic.

Auto-compaction (`slab-agent`'s `maybe_compact`) is persisted by the same single
chain: the observer captures `ContextCompacted` as a `Compacted` line (empty
baseline — the summary is produced async), and the adapter-written next
`TurnState.input_messages` carries the post-compaction baseline on read.

## Line shape

Each line is a flattened JSON object:

```json
{ "timestamp": "2026-08-02T12:00:00Z", "rolloutType": "turnItem", "item": { ... } }
```

`rolloutType` is the adjacent-tag discriminator for [`RolloutItem`] (deliberately
distinct from the inner `"type"` discriminator used by `TurnItem` (camelCase) and
`EventMsg` (snake_case), so the three never collide):

| `rolloutType`     | payload                                  | origin                                   |
| ----------------- | ---------------------------------------- | ---------------------------------------- |
| `sessionMeta`     | `SessionMeta`                            | first line of every file                  |
| `turnItem`        | `slab_agent::protocol::TurnItem`         | `ItemCompleted` (full-fidelity UI)        |
| `eventMsg`        | `slab_agent::protocol::EventMsg`         | turn lifecycle + error/warning (filtered) |
| `compacted`       | `CompactedPayload`                       | context compaction snapshot               |
| `turnContext`     | `TurnContextPayload`                     | LLM-grade `ConversationMessage` deltas    |

## Turn attribution invariant (M5)

`TurnItem` lines carry no explicit `turn_index`; `read_turn_items` attributes each
item from the most recently seen `TurnContext` line (a running turn). This is safe
because `slab-agent` guarantees the turn-N user message (`MessageAppend`, written
directly by the adapter) lands in the file BEFORE any `ItemCompleted` for turn N
(written by the observer after a broadcast hop) — the user message is persisted in
`AgentThread::run` before the `'turns` loop starts `execute_turn`, and both paths
share one FIFO recorder per thread.

**Within a single turn** this is a synchronized guarantee (the user-message append
is awaited before `execute_turn` runs). **Across turns** it is an assumption that
holds in practice rather than a synchronized invariant: there is no per-thread
turn-boundary barrier fencing the observer's drain against the next turn's
`send_input`. The race is negligible because the observer has the whole
turn-teardown + client-roundtrip + `send_input`-setup window to drain one event
before the next turn's `MessageAppend` is appended, but it is not enforced by a
barrier. Fork is the one operation that would deterministically break the ordering
(it batches per-row copies), which is why the harness rebuilds the child
wholesale in correct interleaved order.

## Design notes

- **Single writer per thread** (`RolloutRecorder` mpsc actor, lazy materialization
  — the file is not opened until the first real write).
- **Two-phase recovery**: on a write error the writer is dropped but the pending
  items are retained and retried once after reopening. Middle events are never
  lost.
- **Read path** opens a separate read-only handle; reads flush the writer first.
- **Atomic truncate/rotation** via `tempfile::persist`. The recorder actor drops its
  own write handle before the rename, and a read handle open at the instant of the
  rename may briefly contend on Windows (`ERROR_SHARING_VIOLATION`); the replace
  retries transient sharing violations with a bounded exponential backoff so the
  rename ultimately succeeds.

See the master plan (`crates-slab-agent-tracing-slab-agent-rol-lexical-sky.md`)
for the full architecture and the integration slices (adapter, observer,
migration, backfill) that live in `slab-app-core`.
