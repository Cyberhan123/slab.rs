# slab-mcp

MCP protocol helpers for Slab.

## Role

`slab-mcp` owns the minimal MCP JSON-RPC protocol types, stdio client connection, cached tool discovery, tool calls, and exposing a `ToolRouter` through MCP server responses.

It does not register tools by itself. Host layers or `slab-agent-tools` decide which MCP clients are injected into an agent.

## Type

Rust library crate.

## Testing

- Run the crate test suite with `cargo test -p slab-mcp` from the repo root.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
