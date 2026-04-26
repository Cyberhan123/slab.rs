# Project Guidelines

Keep repo guidance high signal and task-focused. Prefer concrete local context over broad exploration.

If a task is blocked by missing product intent, missing access, or conflicting user changes, ask the user. Otherwise continue autonomously.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

## Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No speculative error handling for scenarios that have no evidence in the current code path.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

## Bug fix 

When fixing bugs, it's essential to start with first principles to find the root cause. You can't compromise the fundamental problem by making minimal fixes.

## Workflow
- When docs and implementation disagree, verify the current behavior in code before changing docs or behavior.
- Keep repo guidance deterministic: commands should work from the repo root, and local references should come before remote-only advice.
- `.agents/skills` contains optional task guidance. `plugins/` contains runtime plugin packages, `manifests/` contains JSON schemas and model metadata, and `vendor/` contains vendored runtime artifacts.
- Do not add repo-specific workflow that forces a skill-selection step for every task. Use skills when the active environment requires them or when the task directly matches them.
- When updating repo guidance, confirm the current workspace members and scripts instead of copying older architecture notes forward.

## Hard Constraints

- Keep inference behind `bin/slab-app -> bin/slab-server -> crates/slab-app-core runtime supervisor -> GrpcGateway -> bin/slab-runtime -> crates/slab-runtime-core`; the desktop host starts `slab-server`, product API traffic stays on HTTP, and Tauri commands stay host-only.
- Extend the existing `/v1/*` API surface instead of adding a parallel API tree.
- Keep long-running AI work in task-oriented flows when the feature already follows that model.
- Prefer `crates/slab-types` and `crates/slab-proto` for contracts that cross crate boundaries.
- Keep `crates/slab-app-core` HTTP-free, keep `bin/slab-runtime` as the only runtime composition root, and keep `crates/slab-runtime-core` limited to scheduler/backend protocol concerns.
- Keep `crates/slab-agent` pure; built-in deterministic tools belong in `crates/slab-agent-tools`, and plugin/API capability adapters are registered by host/app-core layers.
- Preserve Tauri CSP, capabilities, permissions, sidecar boundaries, and plugin sandboxing unless the task explicitly changes them.
- Keep Tauri child WebViews as the default third-party plugin UI runtime; do not make Module Federation the default plugin model.
- Keep `plugin.json` as the static source of truth. `manifestVersion: 1` separates runtime assets, `contributes.*`, `permissions.*`, and agent capabilities.
- Plugin WebView commands must derive the caller plugin id from the WebView label, not from plugin-supplied payload fields.
- `plugins/` is runtime plugin content, not AI skill content; `.agents/skills` is only for agent guidance.
- SQLx migrations in `crates/slab-app-core/migrations/` are append-only.
- Cargo excludes `packages/slab-desktop/src`, so Rust tooling does not validate the TypeScript frontend.
- When backend API shapes change, regenerate `packages/api/src/v1.d.ts` with `bun run gen:api`.

## Reference Pointers

Keep this file focused on always-on constraints. For module-specific role, stack, testing, and layout details, prefer the nearest subproject `README.md` before restating that information here.

- Desktop host and Tauri backend: `bin/slab-app/src-tauri/README.md`
- HTTP gateway: `bin/slab-server/README.md`
- Runtime worker: `bin/slab-runtime/README.md`
- Windows full installer: `bin/slab-windows-full-installer/README.md`
- Shared business logic: `crates/slab-app-core/README.md`
- Agent control plane: `crates/slab-agent/README.md`
- Built-in agent tools: `crates/slab-agent-tools/README.md`
- Model hub abstraction: `crates/slab-hub/README.md`
- Runtime protocol substrate: `crates/slab-runtime-core/README.md`
- Shared frontend API package: `packages/api/README.md`
- Desktop frontend: `packages/slab-desktop/README.md`
- Shared UI primitives: `packages/slab-components/README.md`
- Plugin author SDK: `packages/slab-plugin-sdk/README.md`
- Plugin workspace and manifest model: `plugins/README.md`
- When a crate or package already has its own `README.md` under `crates/*`, `packages/*`, or `bin/*`, prefer that local README over expanding `AGENTS.md`.

## Common Root Commands

Run these from the repo root unless a local README says otherwise. Pick the narrowest command that validates your change before reaching for broader workspace-wide checks:

```sh
bun install
bun run lint
bun run lint:fix
bun run test
bun run build:desktop
bun run dev:app
bun run gen:api
bun run gen:plugin-packs
bun run gen:schemas
cargo check --workspace
cargo test --workspace
```

For package-specific or crate-specific workflows, prefer the nearest subproject `README.md` and the local `package.json` / `Cargo.toml` scripts over copying those details into this file.

## AI Docs Maintenance

- `AGENTS.md` is the canonical repo-wide AI reference for architecture, boundaries, and build/test commands.
- Keep `AGENTS.md` focused on hard constraints and a short set of repo-root commands; move module-specific reference material into subproject `README.md` files when possible.
- `CLAUDE.md` and `.github/copilot-instructions.md` should stay thin and point to `AGENTS.md` for repo-wide guidance.
- Keep `AGENTS.md` aligned when workflow, architecture snapshot, plugin/runtime boundaries, or build commands change.
- When adding or removing workspace members, plugin surfaces, or desktop sidecar behavior, update this doc and the relevant `README.md` files in the same change.
