# slab-mcp

MCP server management and tool aggregation helpers for Slab.

## Role

`slab-mcp` owns multi-server MCP management for Slab. It keeps configured external MCP servers by name, aggregates their tool lists, caches discovered tools, and routes tool calls to the selected server. Tool-list refresh is per-server tolerant (`refresh_and_probe_servers` reports each server's health) — one dead server no longer fails the whole refresh, and the health map drives the agent runtime's `mcp__*` proxy visibility gating.

It uses `crates/slab-mcp-client` for the low-level single-connection transport. It does not expose Slab as an MCP server; the standalone process entrypoint for that is `bin/slab-mcp-server`.

## Type

Rust library crate.

## Testing

- Run the crate test suite with `cargo test -p slab-mcp` from the repo root.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
