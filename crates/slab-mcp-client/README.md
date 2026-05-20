# slab-mcp-client

Single-connection MCP client transport for Slab.

## Role

`slab-mcp-client` owns the low-level JSON-RPC over stdio connection to one external MCP server. It starts the process, sends `initialize` and `notifications/initialized`, and exposes direct `ping`, `tools/list`, and `tools/call` operations.

It does not know about Slab server names, tool aggregation, permissions, authentication, or caches. Those belong in `crates/slab-mcp`.

## Type

Rust library crate.

## Testing

- Run the crate test suite with `cargo test -p slab-mcp-client` from the repo root.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).

