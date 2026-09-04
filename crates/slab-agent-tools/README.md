# slab-agent-tools

Built-in tool adapters for `slab-agent`.

## Role

`slab-agent-tools` contains host-provided deterministic tools and registration helpers for the Slab agent runtime.

- `slab-agent` keeps the orchestration kernel, tool traits, and routing abstractions.
- `slab-agent-tools` owns concrete built-in tool implementations and the helper that registers them with a `ToolRouter`.
- Shell execution, workspace-safe file operations, Git operations, and MCP protocol handling are delegated to the dedicated support crates.
- Host layers can depend on this crate without moving storage, transport, or business logic into `slab-agent`.
- Built-in tool outputs are bounded by the context-budget system: grep enforces three byte caps (line/match-preview/response), `read_file` caps content at 48 KB, shell output at 30 KB head/tail — every cut carries an explicit omission marker, results past the cap spill their full payload to `.slab/artifacts/<thread_id>/`, and searches prune `.git` / `node_modules` / `vendor` / `dist` / lockfiles by default.
- Subagent delegation is asynchronous by default: `delegate_subagent` returns a background-task id immediately, the detached watcher (tracked in the shared `BackgroundTaskRegistry` as kind `subagent`) delivers the result to the parent via the host's `SubagentTaskSink`, and `subagent_status` / `subagent_message` / `subagent_stop` communicate with a running delegation. `background=false` keeps the legacy inline (blocking) shape.

## Tool authoring

Each tool implements `slab_agent::TypedTool` with a typed `*Args` struct:

- The struct derives `Deserialize + JsonSchema`; field doc comments become schema descriptions, `#[serde(default = "…")]` becomes the schema `default`, and `#[schemars(range/length(…))]` emits `minimum`/`maximum`/`minItems`. Declare fields in the order the model should see them.
- `execute` receives the parsed struct (the blanket `ToolHandler` adapter parses raw arguments once and maps missing fields to the `missing '<field>' argument` wording).
- Validation that must keep a specific model-facing message (alias sets, "must be at least 1" bounds, lenient drop rules) stays in `execute` or a field deserializer; schema-only enum mirrors advertise canonical values without narrowing what parsing accepts.
- Metadata methods (`describe_operation`, `is_concurrency_safe`, `render_turn_item`, …) keep the raw `&Value` arguments on purpose — the dispatch layer calls them where an all-or-nothing typed parse would change behavior.

## Type

Rust library crate.

## Testing

- Run the crate test suite with `cargo test -p slab-agent-tools` from the repo root.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
