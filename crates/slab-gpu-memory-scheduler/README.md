# slab-gpu-memory-scheduler

The single **implementation** of the GPU-memory probing/sizing formulas shared
by the server and the runtime — not a decision authority. Decision rights stay
distributed: eviction/replay = `slab-app-core`'s `ModelAutoUnloadManager`,
`n_ctx` resolution = the runtime engine, compaction = the compact policy. The
crate supplies the math, the telemetry, and the observability those decisions
read.

## Role

- **Probing** — the only place `all-smi` is touched. `GpuProbe` (feature `gpu-telemetry`) collects
  device telemetry; `GpuMemoryScheduler` wraps it with a periodic refresh loop, a last-good cache,
  and deterministic primary-device selection (largest total VRAM, uuid-keyed — multi-GPU ready).
  `gpu_memory_pressure()` exposes the cached primary-gauge fill ratio for
  memory-driven decisions (compaction's pressure gate) without forcing a probe.
- **Sizing math** — `resolve_auto_context` is the one implementation of VRAM-aware `auto` context
  sizing (KV bytes/token, worker multiplier, mmproj reservation, headroom buffer). Consumed by both
  the server (pre-flight estimates) and `bin/slab-runtime` (engine load). The
  tunables (`vram_buffer_bytes`, `auto_context_quantum`, `auto_context_fallback`)
  travel on `GgmlLlamaLoadRequest` (proto fields 9–11, optional); the engine
  falls back per-field to `SchedulerParams::default()` when unset, so server
  and runtime always compute with one policy.
- **Policy** — pure functions for memory-pressure predicates, pressure-eviction candidate
  ordering, and OOM message classification. `slab-app-core`'s `ModelAutoUnloadManager` executes
  them; it owns ref-counting / idle timers / replay / admission, not the math.
- **Ledger + lifecycle hooks** — `ModelLifecycleHook` (ToolHandler-style registry) fires on
  load/unload/inference boundaries; the built-in ledger hook records expected footprints and
  measured probe deltas per device. Decisions always read probe-measured free
  bytes; the ledger is attribution, never ground truth — with one exception:
  the admission pre-check (`evict_until_projected_fit`) feeds its projection
  with probe-measured free bytes, evicting idle residents before dispatch when
  a load cannot fit. Exposed read-only at `/v1/system/gpu/ledger`.

## Boundaries

- Consumed by `slab-app-core` and `bin/slab-runtime`. **Must not depend on `slab-app-core`,
  `slab-agent`, or `slab-config`** (leaf crate; hosts build `SchedulerParams` from their own
  settings).
- Depends on `slab-types` (backend ids), `all-smi` (optional, `gpu-telemetry`), tokio/tracing.
- Hooks fire host-side only — never inside `bin/slab-runtime`; the runtime participates by calling
  the sizing functions and reporting resolved values over the existing gRPC wire.
- `/v1/system/gpu`'s response shape and the `free_vram_bytes` proto field are frozen contracts;
  this crate feeds them, it does not reshape them. `/v1/system/gpu/ledger` is
  the diagnostics-only extension point.

## Behavior notes

- `num_workers > 1` divides the auto-context budget (each worker context allocates its own full
  KV cache), so multi-worker configs resolve smaller `n_ctx` than single-worker ones — by design.
- all-smi reports no `free_memory`; free is derived as `total − used`, and the 2 GiB headroom
  buffer absorbs driver release lag.
- The periodic refresh loop backs off sixfold (5s → 30s by default) while no
  consumer has needed fresh telemetry — load sizing and stale display reads
  reset it to the fast cadence. On an idle laptop the probe stops polling
  WMI/NVML every 5 seconds.
- **mmproj file size is a VRAM-cost proxy, not a measurement** — quantization
  and layer offload can make the resident projector cost diverge from the file
  size. Treat as an estimate until calibrated against measured deltas.
- `after_inference` fires per `ModelUsageGuard` release (once per completed
  request), and the audio/image/video backends reuse the same guard — hook
  implementers see those requests too, not just text generation.
- Unload hook symmetry: IdleTimeout / MemoryPressure / Manual dispatch
  `before_unload` → unload → `after_unload`; RuntimeRestart dispatches a
  *post-hoc* `before_unload` immediately before its `after_unload` (the
  process died — hooks observe, they cannot prevent).
- Compaction's dual gate: the token threshold OR a memory-pressure signal
  (≥ 0.90 fill ratio — injected via `CompactContext::memory_pressure_hint`,
  else the host policy self-queries the scheduler's cached gauge). slab-agent
  stays pure; it only carries the opaque hint.

## Local validation

```sh
cargo test -p slab-gpu-memory-scheduler
cargo clippy -p slab-gpu-memory-scheduler -- -D warnings
```
