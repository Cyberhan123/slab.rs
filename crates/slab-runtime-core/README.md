# slab-runtime-core

Backend worker/thread runtime primitives for Slab.

## Role

`slab-runtime-core` is intentionally limited to the backend-facing execution
substrate used by `bin/slab-runtime/src/infra/backends`. It contains:

- Backend worker ingress and control-bus protocol types.
- Worker handler dispatch helpers used by `slab-runtime-macros`.
- Runtime worker spawning helpers for Tokio tasks and dedicated OS threads.
- Backend admission and worker registration primitives.
- In-process payload and stream chunk envelopes for backend handoff.

Runtime domain concepts such as task orchestration, model resolution,
application errors, HTTP/gRPC status mapping, and public API DTOs do not belong
in this crate. They should live in `bin/slab-runtime`, `bin/slab-server`, or
`crates/slab-app-core` depending on the boundary.

## Type

Rust library crate (backend worker runtime substrate).

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
