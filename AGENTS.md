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
- `tauri-v2`: `bin/slab-app/src-tauri`, sidecar startup, capabilities, commands, plugin webview runtime, and Tauri IPC details

If a task does not clearly match one of the skills above, work directly from the codebase without adding an extra routing layer.

See [docs/ai-skill-map.md](docs/ai-skill-map.md) for the short routing map.

## Architecture Snapshot

This workspace now has a desktop host, supervisor, runtime worker, and separate plugin/agent layers:

```text
bin/slab-app (Tauri desktop host + plugin shell)
    |  \
    |   \-- native IPC (crates/slab-app-core tauri feature)
    |
    | sidecar HTTP
    v
bin/slab-server (HTTP gateway, thin axum layer)
    |
    | (shares crates/slab-app-core)
    v
crates/slab-app-core (business logic: domain, infra, context, config — no HTTP/axum)
    |
    | gRPC / IPC
    v
bin/slab-runtime (worker process: ggml.llama / ggml.whisper / ggml.diffusion)
    |
    v
crates/slab-core (slab-runtime-core: runtime builder, scheduler, engine adapters)
```

- `bin/slab-app` is the Tauri 2 desktop host that launches the `bin/slab-server` sidecar, mounts plugin child webviews, and exposes the desktop-only plugin bridge UI. Its frontend is served from `packages/slab-desktop`. Native IPC commands live in `bin/slab-app/src-tauri/src/api` and delegate directly to `crates/slab-app-core`.
- `packages/slab-desktop` is the React 19 + Vite + React Router 7 frontend application for the Tauri desktop host. It imports UI components from `packages/slab-components` and i18n from `packages/slab-i18n`.
- `packages/slab-components` is the shared shadcn/ui-based React component library (Radix UI + Tailwind CSS). It can be consumed by both `slab-desktop` and future mobile packages.
- `packages/slab-i18n` is the shared internationalization package (i18next + react-i18next) with locale definitions.
- `bin/slab-server` is the thin HTTP gateway and supervisor (axum). It depends on `crates/slab-app-core` for all domain/infra logic; adds axum `FromRef` extractors (`state_extractors.rs`) and `ServerError` → HTTP response conversion. Exposes `/v1` plus `/api-docs/openapi.json`.
- `crates/slab-app-core` is the HTTP-free business logic library: `context/`, `domain/`, `infra/`, `config`, `model_auto_unload`. It is usable both from `bin/slab-server` (HTTP path) and from `bin/slab-app` (native Tauri IPC path). SQLx migrations live in `crates/slab-app-core/migrations/`.
- `bin/slab-runtime` is the standalone gRPC worker that can serve TCP or IPC transports and enable `ggml.llama`, `ggml.whisper`, and `ggml.diffusion` backends independently.
- `crates/slab-core` (package name: `slab-runtime-core`) holds runtime orchestration, scheduler/dispatch logic, and engine adapters. Keep HTTP and SQL concerns out of this crate.
- `crates/slab-agent` is a pure control-plane library for agent threads, tool routing, and port-based orchestration.
- `crates/slab-proto` owns the protobuf contract between `bin/slab-server` and `bin/slab-runtime`.
- `crates/slab-types` is the shared semantic types, settings, runtime, and JSON-schema-friendly contract crate used across the workspace.
- `crates/slab-llama`, `crates/slab-whisper`, `crates/slab-diffusion`, `crates/slab-ggml`, `crates/slab-libfetch`, `crates/slab-subtitle`, `crates/slab-build-utils`, `crates/slab-core-macros`, and the `*-sys` crates provide engine bindings, native/runtime utilities, artifact fetching, and supporting infrastructure.

## Repo Layout

- `bin/slab-server`: thin HTTP gateway; exposes `/v1` routes via axum. Business logic lives in `crates/slab-app-core`.
- `bin/slab-runtime`: gRPC server and runtime worker entry points.
- `crates/slab-app-core`: HTTP-free business logic (domain, infra, context, config). Native IPC wrappers now live in `bin/slab-app/src-tauri/src/api/`. Migrations are in `crates/slab-app-core/migrations/`.
- `crates/slab-core`: runtime library only (package: `slab-runtime-core`); keep HTTP and SQL concerns out.
- `crates/slab-agent`: agent orchestration library and tool router abstractions.
- `crates/slab-types`: shared semantic types, settings models, load specs, and other reusable Rust contracts.
- `crates/slab-proto`: protobuf definitions and generated conversion helpers for server/runtime IPC.
- `packages/slab-desktop/src`: React frontend pages for chat, image, audio, video, hub, plugins, task, setup, settings, and about.
- `packages/slab-desktop/src/pages/chat`: Ant Design X chat UI and page-local wrappers.
- `packages/slab-desktop/src/pages/plugins`: desktop-only plugin center, wasm function bridge, and plugin event viewport UI.
- `packages/slab-desktop/src/lib/plugin-sdk.ts`: frontend bridge to Tauri plugin commands and events.
- `packages/slab-components/src`: shared UI component library (shadcn/ui, Radix UI, Tailwind CSS).
- `packages/slab-i18n/src`: shared i18n setup and locale files.
- `bin/slab-app/src-tauri`: Tauri host, sidecar startup, plugin runtime, capabilities, permissions, and security boundaries.
- `plugins`: local runtime plugin packages loaded by the Tauri host from `plugins/<plugin-id>/`.
- `manifests`: JSON schemas and manifest assets used by settings/model tooling.
- `vendor`: vendored runtime artifacts and external resources kept in-repo.
- `testdata`: sample media, fixture models, and integration assets.

## Working Rules

- Keep inference behind `bin/slab-server -> GrpcGateway -> bin/slab-runtime -> crates/slab-core`; the desktop app can also call `crates/slab-app-core` directly via native Tauri IPC.
- Extend the existing `/v1/*` API modules instead of adding a parallel API tree. The current surface includes `agent`, `audio`, `backend`, `chat`, `ffmpeg`, `images`, `models`, `session`, `settings`, `setup`, `system`, `tasks`, and `video`.
- Keep long-running AI work in task-oriented flows when the feature already follows that model.
- Prefer `crates/slab-types` and `crates/slab-proto` for contracts that need to cross crate boundaries instead of duplicating shapes.
- Keep `crates/slab-agent` pure: storage, HTTP, SSE/WebSocket, and model adapters belong in `crates/slab-app-core` or `bin/slab-server`, behind the port traits.
- Preserve Tauri CSP, capabilities, permissions, sidecar boundaries, and the `plugins/<plugin-id>/plugin.json` contract unless the task explicitly requires a change.
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
cargo make check
cargo make test
cargo make dev
```

Frontend and Tauri:

```sh
# From repo root (bun workspace)
bun install

# Type-check the desktop frontend
cd packages/slab-desktop
bun run build

# Run Tauri development mode
cd bin/slab-app
bun run tauri dev

# Regenerate OpenAPI types
cd packages/slab-desktop
bun run api

# Generate OKLCH color tokens
bun run color:oklch -- background=#f7f9fb primary=#0d9488
```

Server compatibility tests:

```sh
python -m pip install -r bin/slab-server/tests/requirements.txt
pytest bin/slab-server/tests
```

## AI Docs Maintenance

- Keep `AGENTS.md`, `CLAUDE.md`, `.github/copilot-instructions.md`, and `docs/ai-skill-map.md` aligned when the skill list, workflow, architecture snapshot, plugin/runtime boundaries, or build commands change.
- Do not document repo-local skills that do not exist on disk.
- When adding or removing workspace members, plugin surfaces, or desktop sidecar behavior, update this doc set in the same change.
