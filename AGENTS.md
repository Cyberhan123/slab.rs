# Project Guidelines

Use this as the repo-wide AI reference for architecture boundaries, workflow, and documentation ownership. Keep it short; module-specific details belong in the nearest `README.md`.

## Working Rules

- Inspect current code before changing behavior or docs. Implementation wins when docs and code disagree.
- Keep changes minimal and task-scoped. Do not add speculative features, abstractions, or configurability.
- Fix bugs from first principles. Find the root cause instead of masking the symptom with a narrow workaround.
- Touch only what the task requires. Remove only unused code created by your own changes.
- Preserve user or unrelated worktree changes. Mention unrelated issues instead of cleaning them up.

## Workflow Source Of Truth

- The top-level build flow is Bun plus Cargo. Root `package.json` scripts are the canonical daily entrypoints.
- Do not reintroduce `cargo make`, Bazel wrappers, or parallel top-level build orchestration.
- Run commands from the repo root unless a local README says otherwise.
- Use the narrowest validation command that covers the change before reaching for broader workspace checks.
- `.agents/skills` contains optional agent guidance. `plugins/` contains runtime plugin packages. `docs/public/manifests/` contains JSON schemas and model metadata. `vendor/` contains vendored runtime artifacts.

## Architecture Boundaries

- Inference stays behind `bin/slab-app -> bin/slab-server -> crates/slab-app-core runtime supervisor -> GrpcGateway -> bin/slab-runtime -> crates/slab-runtime-core`.
- The desktop host starts `slab-server`; product API traffic stays on HTTP; Tauri commands stay host-only.
- Extend the existing `/v1/*` API surface instead of adding a parallel API tree.
- Backend API shape changes require `bun run gen:api` to refresh `packages/api/src/v1.d.ts`.
- Prefer `crates/slab-types` and `crates/slab-proto` for contracts that cross crate boundaries.
- Keep `crates/slab-app-core` HTTP-free. Keep `bin/slab-runtime` as the runtime composition root. Keep `crates/slab-runtime-core` limited to scheduler and backend protocol concerns.
- Keep `crates/slab-agent` pure. Built-in deterministic tools belong in `crates/slab-agent-tools`; plugin/API capability adapters are registered by host or app-core layers.
- Keep long-running AI work in task-oriented flows when the feature already follows that model.
- Preserve Tauri CSP, capabilities, permissions, sidecar boundaries, and plugin sandboxing unless the task explicitly changes them.
- SQLx migrations in `crates/slab-app-core/migrations/` are append-only.

## Plugin And LSP Boundaries

- Plugin dispatch stays behind `bin/slab-app -> bin/slab-server /v1/plugins/rpc` using WebSocket JSON-RPC 2.0 into `crates/slab-app-core`.
- JS plugin calls go through supervised `bin/slab-js-runtime`; Python plugin calls go through supervised `bin/slab-python-runtime`; WASM/frontend fallback stays behind `crates/slab-plugin`.
- Keep Tauri child WebViews as the default third-party plugin UI runtime. Do not make Module Federation the default plugin model.
- `plugin.json` is the static source of truth. `manifestVersion: 1` separates runtime assets, `contributes.*`, `permissions.*`, and agent capabilities.
- JS plugin runtime calls follow JSON-RPC 2.0 conventions.
- Frontend-only plugins are UI-focused and non-callable; complex plugin logic belongs in JS runtime, Python runtime, or WASM backends.
- Plugin WebView commands must derive the caller plugin id from the WebView label, not plugin-supplied payload fields.
- Workspace LSP traffic stays behind `packages/slab-desktop -> bin/slab-server /v1/workspace/lsp/* -> crates/slab-app-core`.
- `crates/slab-app-core` owns LSP provider resolution and process spawning. The desktop host must not add a second LSP bridge.
- `plugins/web-language-servers` is a build-only package that emits `resources/libs/language-servers/web/*.mjs`; it is not a user-installable plugin.
- Native workspace LSP providers resolve tools such as `rust-analyzer`, `gopls`, and `pyright-langserver` from existing search paths or `PATH`; do not bundle those binaries unless the task explicitly changes this.

## README Ownership

- Every Cargo workspace member and every Bun workspace package must have a local `README.md`.
- Each README should cover role, package/crate type, local validation commands, and hard boundaries for that module.
- Keep `AGENTS.md` focused on always-on constraints. Move module-specific role, stack, testing, and layout details to local README files.
- When adding or removing workspace members, plugin surfaces, sidecars, or package/crate responsibilities, update this file and the affected local README files in the same change.
- `CLAUDE.md` and `.github/copilot-instructions.md` should stay thin and point to `AGENTS.md` for repo-wide guidance.

## Common Root Commands

```sh
bun install

bun run dev:app
bun run dev:desktop

bun run lint
bun run lint:fix
bun run lint:rust

bun run check
bun run check:frontend
bun run check:rust

bun run test
bun run test:frontend
bun run test:rust
bun run test:rust:cargo
bun run test:browser
bun run test:components
bun run test:server

bun run build:desktop
bun run build:language-servers
bun run build:sidecars
bun run build:sidecars:release
bun run build:app
bun run build:windows-installer

bun run gen:api
bun run gen:schemas
bun run gen:plugin-packs
bun run gen:model-packs

bun run docs:dev
bun run docs:build
bun run docs:preview
```

## Reference Map

Start with the nearest local README for the code you are changing.

- Build, generation, and packaging flow: `docs/development/guides/build.md`
- Desktop host and Tauri backend: `bin/slab-app/README.md` and `bin/slab-app/src-tauri/README.md`
- HTTP gateway: `bin/slab-server/README.md`
- Runtime worker: `bin/slab-runtime/README.md`
- JS and Python plugin runtimes: `bin/slab-js-runtime/README.md`, `bin/slab-python-runtime/README.md`
- Shared business logic: `crates/slab-app-core/README.md`
- Runtime protocol substrate: `crates/slab-runtime-core/README.md`
- Agent control plane and tools: `crates/slab-agent/README.md`, `crates/slab-agent-tools/README.md`
- Plugin model and packaging: `plugins/README.md`, `crates/slab-plugin/README.md`, `packages/slab-plugin-sdk/README.md`, `packages/slab-plugin-cli/README.md`, `packages/slab-plugin-ui/README.md`
- Desktop frontend and UI packages: `packages/slab-desktop/README.md`, `packages/slab-components/README.md`, `packages/slab-i18n/README.md`
- Shared contracts and generated clients: `crates/slab-types/README.md`, `crates/slab-proto/README.md`, `packages/api/README.md`
