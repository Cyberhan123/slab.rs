# slab-mcp-server

Standalone MCP server process for exposing Slab capabilities to external AI clients.

## Role

`slab-mcp-server` speaks JSON-RPC over stdio: `initialize`, `ping`, `tools/list`, and `tools/call`. It exposes `slab_server_info` plus the read-only memory workspace tools (`memory_list_projects`, `memory_list`, `memory_search`, `memory_read`) implemented in the `crates/slab-mcp` middle layer. The memory root defaults to `<app_home>/memories` and can be overridden with the `SLAB_MEMORIES_ROOT` environment variable.

It does not link `slab-app-core` or `slab-agent`. Real Slab tools should be added through the `crates/slab-mcp` middle layer rather than directly in this process entrypoint.

## Type

Rust binary crate.

## Testing

- Run the crate test suite with `cargo test -p slab-mcp-server` from the repo root.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).

