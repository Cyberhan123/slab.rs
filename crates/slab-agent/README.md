# slab-agent

Agent orchestration library for Slab.

## Role

`slab-agent` is a pure control-plane library that provides:

- Agent thread management and lifecycle control.
- Tool routing and port-based orchestration abstractions.
- Interfaces for composing multi-step AI workflows.

Storage, HTTP transport, SSE/WebSocket, and model adapters are intentionally kept outside this crate and belong in `crates/slab-app-core` or `bin/slab-server`.

## Type

Rust library crate.

## License

AGPL-3.0-only. See [LICENSE](./LICENSE).
