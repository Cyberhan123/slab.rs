# slab-mcp-client

Single-connection MCP client transport for Slab.

## Role

`slab-mcp-client` owns the low-level JSON-RPC over stdio connection to one external MCP server. It starts the process, sends `initialize` and `notifications/initialized`, and exposes direct `ping`, `tools/list`, and `tools/call` operations.

It does not know about Slab server names, tool aggregation, permissions, authentication, or caches. Those belong in `crates/slab-mcp`.

## Process-tree containment (S6c)

`sandbox.rs` ensures the spawned MCP server (and any process it forks) is torn down when the client drops — previously the server was orphaned on shutdown. It applies reliable tree teardown: a Windows Job Object (`KILL_ON_JOB_CLOSE`) on Windows, or a process-group `SIGKILL` on Unix, plus `kill_on_drop(true)` on the command. Network policy is intentionally unchanged — MCP servers are long-lived and normally need outbound network. The Job helper is inlined here (no dependency on `slab-windows-sandbox`) to keep this lightweight client free of the WFP/ACL/elevation dependency graph.

## Type

Rust library crate.

## Testing

- Run the crate test suite with `cargo test -p slab-mcp-client` from the repo root.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).

