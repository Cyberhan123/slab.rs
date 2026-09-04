# slab-agent

Agent orchestration library for Slab.

## Role

`slab-agent` is a pure control-plane library that provides:

- Agent thread management and lifecycle control.
- Tool routing and port-based orchestration abstractions.
- Typed tool authoring: `TypedTool` declares the argument shape once — an `Input` type deriving `Deserialize + JsonSchema` drives both the model-facing parameters schema and argument parsing, and a blanket impl adapts every `TypedTool` to the `ToolHandler` the `ToolRouter` dispatches on. Tools with a remote/plugin-authored schema (MCP and plugin proxies) use `Input = serde_json::Value` and override `parameters_schema`.
- `ToolResultGuard`, the run-scoped context-budget choke point: every dispatched tool result is bounded to a 64 KB net cap (70/30 head/tail middle-truncation) and identical results ≥ 2 KB are deduplicated by hash, so no single tool call can crowd out the context window.
- Two-tier history compaction: a deterministic micro tier stubs old tool results at 0.55×W (progressively, down to 0.45×W, never touching `delegate_subagent` conclusions), escalating to an LLM summarize at 0.80×W; `CompactOutcome` reports both removed and stubbed counts.
- Approval hooks for sensitive tool calls; host layers provide the approval transport.
- Interfaces for composing multi-step AI workflows.

Storage, HTTP transport, SSE/WebSocket, and model adapters are intentionally kept outside this crate and belong in `crates/slab-app-core` or `bin/slab-server`.

## Type

Rust library crate.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
