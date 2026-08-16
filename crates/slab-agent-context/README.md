# slab-agent-context

Agent context management for slab.rs. This crate discovers skills and
`AGENTS.md` sources and renders the system / developer / user instruction
fragments that frame an agent thread.

## Role

- Pure library. Owns context discovery (skills under `.agents/skills` and the
  global app-home `skills/`; workspace and global `AGENTS.md`) and instruction
  rendering via minijinja.
- Ships an `slab_agent::AgentHook` ([`ContextInstructionHook`]) that injects
  the rendered context on agent start, plus an [`AgentContextSources`] port for
  the dynamic inputs (workspace root, app-home paths, model instruction
  template) that the host supplies.
- A model-provided `instruction_template.jinja` (threaded from
  `slab-model-pack` through `slab-app-core`) overrides the bundled default
  developer template.

## Boundaries

- No HTTP, no storage, no transport. Hosts register the hook and supply the
  sources; skill-body expansion of invoked skills happens in `slab-server`
  (hooks can only inject new messages, not mutate user turns).

## Local validation

```sh
cargo test -p slab-agent-context
cargo build -p slab-agent-context
bun run lint:rust
```
