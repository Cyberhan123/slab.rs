# slab-agent

Agent orchestration library for Slab.

## Role

`slab-agent` is a pure control-plane library that provides:

- Agent thread management and lifecycle control.
- Tool routing and port-based orchestration abstractions.
- Typed tool authoring: `TypedTool` declares the argument shape once — an `Input` type deriving `Deserialize + JsonSchema` drives both the model-facing parameters schema and argument parsing, and a blanket impl adapts every `TypedTool` to the `ToolHandler` the `ToolRouter` dispatches on. Tools with a remote/plugin-authored schema (MCP and plugin proxies) use `Input = serde_json::Value` and override `parameters_schema`.
- Reversible tool lifecycle + dependency gating: `ToolRouter::register` returns a `ToolRegistration` handle (explicit `dispose()`, no Drop semantics) and disposes replaced/removed handlers via `ToolHandler::dispose` outside the registry locks (bulk removals in reverse registration order); `ToolHandler::service_deps` declares host-service keys (`ToolServiceKey`) that gate the model-facing projections — unsatisfied tools stay registered and dispatchable but PENDING (hidden) until the host marks the key satisfied via `set_dep_satisfied`.
- `ToolResultGuard`, the run-scoped context-budget choke point: every dispatched tool result is bounded to a 64 KB net cap (70/30 head/tail middle-truncation) and identical results ≥ 2 KB are deduplicated by hash, so no single tool call can crowd out the context window.
- Two-tier history compaction: a deterministic micro tier stubs old tool results at 0.55×W (progressively, down to 0.45×W, never touching `delegate_subagent` conclusions), escalating to an LLM summarize at 0.80×W; `CompactOutcome` reports both removed and stubbed counts.
- Approval hooks for sensitive tool calls; host layers provide the approval transport.
- Per-thread workspace roots: `AgentConfig::workspace_root` (persisted in the
  thread's `config_json`) overrides the shared thread-context workspace for
  that thread — the carrier for global chat sessions, whose file tools root
  at a per-session artifacts directory instead of the process cwd. Resumes
  re-read it from the persisted config; `spawn_child_for_parent` inherits it
  into subagent configs; `AgentControl::set_thread_workspace_root` re-applies
  it per turn like the other config overrides.
- Interfaces for composing multi-step AI workflows.

Storage, HTTP transport, SSE/WebSocket, and model adapters are intentionally kept outside this crate and belong in `crates/slab-app-core` or `bin/slab-server`.

## Type

Rust library crate.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
