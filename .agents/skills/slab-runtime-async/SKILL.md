---
name: slab-runtime-async
description: Work on slab-core and slab-runtime async internals. Use for orchestrator, pipeline, stages, backend runners, task flow, channels, and concurrency-sensitive runtime behavior in this repository.
---

# Slab Runtime Async

Use this skill for concurrency-sensitive work in:

- `slab-core/src/runtime/**`
- `slab-runtime/src/grpc/**`
- backend runner and admission logic
- orchestrator, pipeline, stage, storage, and runtime control flow

## Repo Defaults

- Tokio is the async runtime.
- The project already has runtime abstractions for orchestrator, pipeline, stages, storage, and backend protocols.
- Generic async guidance should support those abstractions, not replace them.

## Workflow

1. Read the nearest runtime abstraction before inventing a new async flow.
2. Prefer extending orchestrator, pipeline, stage, storage, or backend protocol code over adding ad hoc task spawning.
3. If you need general async patterns, open `../rust-async-patterns/SKILL.md`.
4. Keep cancellation, backpressure, admission control, and task lifecycle explicit.
5. When behavior changes, look for the closest runtime test location and add coverage there.

## Avoid

- Do not block async code.
- Do not bypass existing runtime abstractions with unrelated channels or detached tasks unless there is a strong reason.
- Do not leak server-side concerns into `slab-core` or `slab-runtime`.
- Do not break typestate-style guarantees in the pipeline builder without replacing them with something equally safe.

## Useful Files

- `slab-core/src/runtime/orchestrator.rs`
- `slab-core/src/runtime/pipeline.rs`
- `slab-core/src/runtime/stage.rs`
- `slab-core/src/runtime/storage.rs`
- `slab-core/src/runtime/backend/**`
- `slab-runtime/src/grpc/**`

## Done When

- The new flow fits existing runtime abstractions.
- Backpressure, cancellation, and lifecycle semantics stay understandable.
- Tests cover the new concurrency behavior where practical.
