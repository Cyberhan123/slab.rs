# slab-agent-tools

Built-in tool adapters for `slab-agent`.

## Role

`slab-agent-tools` contains host-provided deterministic tools and registration helpers for the Slab agent runtime.

- `slab-agent` keeps the orchestration kernel, tool traits, and routing abstractions.
- `slab-agent-tools` owns concrete built-in tool implementations and the helper that registers them with a `ToolRouter`.
- Shell execution, workspace-safe file operations, Git operations, and MCP protocol handling are delegated to the dedicated support crates.
- Host layers can depend on this crate without moving storage, transport, or business logic into `slab-agent`.
- Built-in tool outputs are bounded by the context-budget system: grep enforces three byte caps (line/match-preview/response), `read_file` caps content at 48 KB, shell output at 30 KB head/tail — every cut carries an explicit omission marker, results past the cap spill their full payload to `.slab/artifacts/<thread_id>/`, and searches prune `.git` / `node_modules` / `vendor` / `dist` / lockfiles by default.

## Type

Rust library crate.

## Testing

- Run the crate test suite with `cargo test -p slab-agent-tools` from the repo root.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
