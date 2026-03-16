---
name: slab-runtime-async
description: Work on slab-core scheduler and slab-runtime async internals. Use for orchestrator, pipeline, stages, backend runners, task flow, channels, and concurrency-sensitive runtime behavior in this repository.
---

# Slab Runtime Async

Use this skill for concurrency-sensitive work in:

- `slab-core/src/scheduler/**`
- `slab-runtime/src/grpc/**`
- backend runner and admission logic
- orchestrator, pipeline, stage, storage, and scheduler control flow

## Repo Defaults

- Tokio is the async runtime.
- The project already has scheduler abstractions for orchestrator, pipeline, stages, storage, and backend protocols.
- Generic async guidance should support those abstractions, not replace them.

## Workflow

1. Read the nearest scheduler abstraction before inventing a new async flow.
2. Prefer extending orchestrator, pipeline, stage, storage, or backend protocol code over adding ad hoc task spawning.
3. Keep cancellation, backpressure, admission control, and task lifecycle explicit.
4. When behavior changes, look for the closest runtime test location and add coverage there.

## Async Guardrails

- Use `JoinSet` when a caller should own a task group and collect results as tasks finish.
- Use `tokio::select!` when cancellation, shutdown, or first-finished wins matters.
- Use `mpsc`, `broadcast`, `watch`, and `oneshot` intentionally based on ownership and fan-out needs.
- Put explicit concurrency limits around fan-out work with semaphores or bounded stream buffering.
- Prefer typed errors at stable crate boundaries and add context at application edges.
- Make shutdown observable with tracing and give long-lived tasks an explicit cancellation path.

## Avoid

- Do not block async code.
- Do not bypass existing runtime abstractions with unrelated channels or detached tasks unless there is a strong reason.
- Do not leak server-side concerns into `slab-core` or `slab-runtime`.
- Do not break typestate-style guarantees in the pipeline builder without replacing them with something equally safe.
- Do not hold async locks across unrelated awaits.
- Do not spawn unbounded work without an admission policy or concurrency limit.

## Useful Files

- `slab-core/src/scheduler/orchestrator.rs`
- `slab-core/src/scheduler/pipeline.rs`
- `slab-core/src/scheduler/stage.rs`
- `slab-core/src/scheduler/storage.rs`
- `slab-core/src/scheduler/backend/**`
- `slab-runtime/src/grpc/**`

## Done When

- The new flow fits existing scheduler abstractions.
- Backpressure, cancellation, and lifecycle semantics stay understandable.
- Tests cover the new concurrency behavior where practical.
