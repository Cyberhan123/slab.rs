# slab-mcp-server

Standalone MCP server process for exposing Slab capabilities to external AI clients.

## Role

`slab-mcp-server` speaks JSON-RPC over stdio. The first version is a protocol shell: it supports `initialize`, `ping`, and `tools/list`, returns an empty tool list, and reports `tools/call` as a tool-not-found error.

It does not link `slab-app-core` or `slab-agent`. Real Slab tools should be added through the `crates/slab-mcp` middle layer rather than directly in this process entrypoint.

## Type

Rust binary crate.

## Testing

- Run the crate test suite with `cargo test -p slab-mcp-server` from the repo root.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).

