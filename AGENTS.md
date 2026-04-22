# Project Guidelines

## Agent Workflow

- Read this file before making changes.
- Trust the current code when docs and implementation disagree, then update the docs.
- Keep repo guidance deterministic: commands should work from the repo root, and local references should come before remote-only advice.
- `.agents/skills` contains optional task guidance. `plugins/` contains runtime plugin packages, `manifests/` contains JSON schemas and model metadata, and `vendor/` contains vendored runtime artifacts.
- The project is stable now, so do not force a skill-selection step for every task. Read the code first and only open a skill when the task directly matches it.
- When updating repo guidance, confirm the current workspace members and scripts instead of copying older architecture notes forward.

## Local Skills

Only the skills that actually exist in `.agents/skills` should be treated as repo-local defaults:

- `use-x-chat`: chat state and `useXChat` / `useXConversations` work in `packages/slab-desktop/src/pages/chat/**`
- `x-request`: frontend chat transport changes in `packages/slab-desktop/src/pages/chat/chat-context.ts`
- `x-markdown`: assistant Markdown rendering in `packages/slab-desktop/src/pages/chat/components/chat-message-list.tsx`
- `x-chat-provider`: custom Ant Design X provider work, only when the built-in `DeepSeekChatProvider` no longer fits the backend contract
- `shadcn-ui`: shared React UI primitives, forms, and Tailwind-based component patterns (in `packages/slab-components/`)
- `tauri-v2`: `bin/slab-app/src-tauri`, sidecar startup, capabilities, commands, plugin webview runtime, and desktop host integration details

If a task does not clearly match one of the skills above, work directly from the codebase without adding an extra routing layer.

See [docs/development/ai-skill-map.md](docs/development/ai-skill-map.md) for the short routing map.

## Architecture Snapshot

This workspace now has desktop and headless hosts that share a core runtime supervisor, plus separate runtime worker and plugin/agent layers:

```text
bin/slab-app (Tauri desktop host + plugin shell)
    | local HTTP + Tauri sidecar adapter
    v
bin/slab-server (axum HTTP gateway, desktop/headless host)
    |
    | app-core domain/services + shared runtime supervisor
    v
crates/slab-app-core (business logic + shared runtime supervisor -- no HTTP/axum/Tauri)
    |
    | gRPC / IPC
    v
GrpcGateway / runtime endpoints
    |
    | gRPC / IPC
    v
bin/slab-runtime (worker process: backend composition root)
    |
    v
crates/slab-runtime-core (scheduler, backend protocol, worker runner)
```

- `bin/slab-app` is the Tauri 2 desktop host. It mounts plugin child webviews, exposes the desktop-only plugin bridge UI, starts `bin/slab-server` as a local sidecar, and keeps the main frontend data path on HTTP instead of native business-API IPC.
- `packages/slab-desktop` is the React 19 + Vite + React Router 7 frontend application for the Tauri desktop host. It imports UI components from `packages/slab-components` and i18n from `packages/slab-i18n`.
- `packages/slab-components` is the shared shadcn/ui-based React component library (Radix UI + Tailwind CSS). It can be consumed by both `slab-desktop` and future mobile packages.
- `packages/slab-i18n` is the shared internationalization package (i18next + react-i18next) with locale definitions.
- `packages/vitest-rust-reporter` is a workspace helper package that adapts `cargo test` and optional `cargo llvm-cov` output into a Vitest project so Rust results appear in `vitest --ui`.
- `bin/slab-server` is the thin HTTP gateway and headless host (axum). It depends on `crates/slab-app-core` for all domain/infra logic, adds axum `FromRef` extractors (`state_extractors.rs`) and `ServerError` → HTTP response conversion, and launches `bin/slab-runtime` through the same shared supervisor using a `tokio::process` adapter. Runtime crashes restart per backend; the HTTP host stays up unless the gateway itself fails. It exposes `/v1` plus `/api-docs/openapi.json`.
- `crates/slab-app-core` is the HTTP-free business logic library: `context/`, `domain/`, `infra/`, `config`, `model_auto_unload`, and `runtime_supervisor`. It is the shared domain/runtime layer behind `bin/slab-server`. SQLx migrations live in `crates/slab-app-core/migrations/`.
- `bin/slab-runtime` is the standalone gRPC worker that can serve TCP or IPC transports. It is the only backend composition root: it owns driver resolution, load/inference codecs, task submission, and now keeps its DDD-style `api/`, `application/`, `domain/`, `infra/`, and `bootstrap/` layers in-package, with system startup in `src/bootstrap/`, gRPC handlers in `src/api/handlers/`, application orchestration in `src/application/services/`, and flattened GGML, Candle, and ONNX backend implementations under `src/infra/backends/`.
- `bin/slab-windows-full-installer` is the Windows-only outer bootstrap packer/runtime. It builds the self-extracting full installer EXE, embeds the resource-less Tauri NSIS `setup.exe` plus CAB payloads, expands runtime payloads into `%TEMP%`, and lets NSIS complete the actual app install/uninstall work.
- `crates/slab-runtime-core` (package name: `slab-runtime-core`) now holds only the scheduler, backend protocol, worker runner, task state, common error surface, and generic payload types. Keep HTTP, SQL, typed inference codecs, and backend composition concerns out of this crate.
- `crates/slab-agent` is a pure control-plane library for agent threads, tool routing, and port-based orchestration.
- `crates/slab-agent-tools` contains host-provided deterministic tool handlers and tool registration helpers for `slab-agent`; keep business/tool implementations out of `crates/slab-agent`.
- `crates/slab-proto` owns the protobuf contract between `bin/slab-server` and `bin/slab-runtime`.
- `crates/slab-types` is the shared semantic types, settings, runtime, and JSON-schema-friendly contract crate used across the workspace.
- `crates/slab-llama`, `crates/slab-whisper`, `crates/slab-diffusion`, `crates/slab-ggml`, `crates/slab-libfetch`, `crates/slab-subtitle`, `crates/slab-build-utils`, `crates/slab-runtime-macros`, and the `*-sys` crates provide engine bindings, low-level runtime utilities, artifact fetching, and supporting infrastructure.

## Repo Layout

- `bin/slab-server`: thin HTTP gateway; exposes `/v1` routes via axum. Business logic lives in `crates/slab-app-core`.
- `bin/slab-runtime`: gRPC server and runtime worker package. `src/main.rs` is the thin binary entrypoint; `src/api`, `src/application`, `src/domain`, and `src/infra` hold the worker logic and backend composition.
- `bin/slab-windows-full-installer`: Windows full-installer bootstrap binary. `pack` builds the outer self-extracting installer, `run` expands CAB payloads and launches the embedded Tauri NSIS installer, and `apply` is the helper entrypoint used by NSIS hooks to copy `resources/libs`.
- `crates/slab-app-core`: HTTP-free business logic (domain, infra, context, config) plus the shared runtime supervisor used by `slab-server`. Migrations are in `crates/slab-app-core/migrations/`.
- `crates/slab-hub`: unified model hub abstraction used by `slab-app-core` for feature-gated Hugging Face / ModelScope-style listing, download, and provider fallback.
- `crates/slab-runtime-core`: pure scheduler/backend-protocol library only (package: `slab-runtime-core`); keep HTTP, SQL, driver resolution, typed codecs, and backend composition concerns out.
- `bin/slab-runtime/src/infra/backends`: in-package GGML, Candle, and ONNX backend registrations, engines, adapters, and worker implementations.
- `crates/slab-agent`: pure agent orchestration library and tool router abstractions.
- `crates/slab-agent-tools`: built-in deterministic agent tools and registration helpers used by app-core.
- `crates/slab-types`: shared semantic types, settings models, load specs, and other reusable Rust contracts.
- `crates/slab-proto`: protobuf definitions and generated conversion helpers for server/runtime IPC.
- `packages/slab-desktop/src`: React frontend pages for chat, image, audio, video, hub, plugins, task, setup, settings, and about.
- `packages/slab-desktop/src/pages/chat`: Ant Design X chat UI and page-local wrappers.
- `packages/slab-desktop/src/pages/plugins`: desktop-only plugin center, wasm function bridge, and plugin event viewport UI.
- `packages/slab-desktop/src/lib/plugin-sdk.ts`: frontend bridge to Tauri plugin commands and events.
- `packages/slab-components/src`: shared UI component library (shadcn/ui, Radix UI, Tailwind CSS).
- `packages/slab-i18n/src`: shared i18n setup and locale files.
- `packages/vitest-rust-reporter/src`: Vitest-side Rust test and coverage projection helpers for the workspace test UI.
- `bin/slab-app/src-tauri`: Tauri host, sidecar startup, plugin runtime, capabilities, permissions, and security boundaries.
- `docs`: public VitePress site source.
- `docs/development`: internal planning, audits, engineering notes, AI maintenance docs, and contributor-only references.
- `docs/public/manifests/v1`: published schemars-generated JSON Schemas served at `https://slab.reorgix.com/manifests/v1/*`.
- `plugins`: local runtime plugin packages loaded by the Tauri host from `plugins/<plugin-id>/`; plugin manifests can declare runtime assets, extension contributions, permissions, and agent capabilities.
- `manifests`: JSON schemas and manifest assets used by settings/model tooling.
- `vendor`: vendored runtime artifacts and external resources kept in-repo.
- `testdata`: sample media, fixture models, and integration assets.

## Working Rules

- Keep inference behind `host -> bin/slab-server -> crates/slab-app-core runtime supervisor -> GrpcGateway -> bin/slab-runtime local composition layer -> crates/slab-runtime-core scheduler/backend protocol`; the desktop host should launch `slab-server` and keep product API traffic on HTTP.
- Extend the existing `/v1/*` API modules instead of adding a parallel API tree. The current surface includes `agent`, `audio`, `backend`, `chat`, `ffmpeg`, `images`, `models`, `session`, `settings`, `setup`, `system`, `tasks`, and `video`.
- Keep long-running AI work in task-oriented flows when the feature already follows that model.
- Prefer `crates/slab-types` and `crates/slab-proto` for contracts that need to cross crate boundaries instead of duplicating shapes.
- Keep `crates/slab-agent` pure: storage, HTTP, SSE/WebSocket, model adapters, and concrete tool implementations belong outside it. Put built-in deterministic tools in `crates/slab-agent-tools`; plugin and API tool adapters are registered by host/app-core layers.
- Preserve Tauri CSP, capabilities, permissions, sidecar boundaries, and the `plugins/<plugin-id>/plugin.json` contract unless the task explicitly requires a change.
- Treat plugin `manifestVersion: 1` as a declaration of runtime assets, `contributes.*`, `permissions.*`, and agent capabilities. MCP is an export target for capabilities, not the plugin runtime itself.
- `plugins/` is runtime plugin content, not AI skill content; `.agents/skills` is only for agent guidance.
- SQLx migrations in `crates/slab-app-core/migrations/` are append-only.
- Cargo excludes `packages/slab-desktop/src`, so Rust tooling does not validate the TypeScript frontend.
- When backend API shapes change, regenerate `packages/slab-desktop/src/lib/api/v1.d.ts` from `http://localhost:3000/api-docs/openapi.json`.

## Build and Test

From the repo root:

```sh
cargo build --workspace
cargo test --workspace
cargo check --workspace
cargo check -p slab-server
cargo check -p slab-runtime
cargo check -p slab-agent-tools
cargo check -p slab-windows-full-installer
cargo make check
cargo make test
cargo make dev
cargo make build-windows-full-installer
```

Frontend and Tauri:

```sh
# From repo root (bun workspace)
bun install

# Run frontend/workspace lint from the repo root
bun run lint

# Apply Oxlint autofixes where available
bun run lint:fix

# Run all Vitest projects from the repo root
bun run test:run

# Run Vitest coverage across all configured projects
bun run test:coverage

# Run a single Vitest project from the repo root
bun run test:desktop
bun run test:server

# Type-check the desktop frontend
cd packages/slab-desktop
bun run build

# Run Tauri development mode
cd bin/slab-app
bun run tauri dev

# Build the Windows full installer bootstrap + NSIS bundle
cd ../..
cargo make build-windows-full-installer

# Regenerate OpenAPI types
cd packages/slab-desktop
bun run api

# Refresh published docs schema assets
cd ../..
bun run docs:schemas

# Generate OKLCH color tokens
bun run color:oklch -- background=#f7f9fb primary=#0d9488
```

Server compatibility tests:

```sh
python -m pip install -r bin/slab-server/tests/requirements.txt
pytest bin/slab-server/tests
```

## AI Docs Maintenance

- Keep `AGENTS.md`, `CLAUDE.md`, `.github/copilot-instructions.md`, and `docs/development/ai-skill-map.md` aligned when the skill list, workflow, architecture snapshot, plugin/runtime boundaries, or build commands change.
- Do not document repo-local skills that do not exist on disk.
- When adding or removing workspace members, plugin surfaces, or desktop sidecar behavior, update this doc set in the same change.
