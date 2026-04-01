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

- `use-x-chat`: chat state and `useXChat` / `useXConversations` work in `slab-app/src/pages/chat/**`
- `x-request`: frontend chat transport changes in `slab-app/src/pages/chat/chat-context.ts`
- `x-markdown`: assistant Markdown rendering in `slab-app/src/pages/chat/components/chat-message-list.tsx`
- `x-chat-provider`: custom Ant Design X provider work, only when the built-in `DeepSeekChatProvider` no longer fits the backend contract
- `shadcn-ui`: shared React UI primitives, forms, and Tailwind-based component patterns
- `tauri-v2`: `slab-app/src-tauri`, sidecar startup, capabilities, commands, plugin webview runtime, and Tauri IPC details

If a task does not clearly match one of the skills above, work directly from the codebase without adding an extra routing layer.

See [docs/ai-skill-map.md](docs/ai-skill-map.md) for the short routing map.

## Architecture Snapshot

This workspace now has a desktop host, supervisor, runtime worker, and separate plugin/agent layers:

```text
slab-app (Tauri desktop host + plugin shell)
    |  \
    |   \-- native IPC (slab-app-core tauri feature)
    |
    | sidecar HTTP
    v
slab-server (HTTP gateway, thin axum layer)
    |
    | (shares slab-app-core)
    v
slab-app-core (business logic: domain, infra, context, config — no HTTP/axum)
    |
    | gRPC / IPC
    v
slab-runtime (worker process: ggml.llama / ggml.whisper / ggml.diffusion)
    |
    v
slab-runtime-core (runtime builder, scheduler, engine adapters)
```

- `slab-app` is the React 19 + Vite + React Router 7 + Tauri 2 desktop host. It launches the `slab-server` sidecar, mounts plugin child webviews, and exposes the desktop-only plugin bridge UI. It also embeds `slab-app-core` via the `tauri` feature for native IPC commands.
- `slab-app-core` is the HTTP-free business logic library: `context/`, `domain/`, `infra/`, `config`, `model_auto_unload`. It is usable both from `slab-server` (HTTP path) and from `slab-app` (native Tauri IPC path). SQLx migrations live in `slab-app-core/migrations/`. Enable the `tauri` feature to expose `slab_app_core::tauri_bridge::register()`.
- `slab-server` is the thin HTTP gateway and supervisor. It re-exports `slab-app-core` domain and infra types and adds the axum `FromRef` extractors (`state_extractors.rs`) and HTTP error conversion (`ServerError`).
- `slab-runtime` is the standalone gRPC worker that can serve TCP or IPC transports and enable `ggml.llama`, `ggml.whisper`, and `ggml.diffusion` backends independently.
- `slab-runtime-core` (directory: `slab-core/`) holds runtime orchestration, scheduler/dispatch logic, and engine adapters. Keep HTTP and SQL concerns out of this crate.
- `slab-agent` is a pure control-plane library for agent threads, tool routing, and port-based orchestration.
- `slab-proto` owns the protobuf contract between `slab-server` and `slab-runtime`.
- `slab-types` is the shared semantic types, settings, runtime, and JSON-schema-friendly contract crate used across the workspace.
- `slab-llama`, `slab-whisper`, `slab-diffusion`, `slab-ggml`, `slab-libfetch`, `slab-subtitle`, `slab-build-utils`, `slab-core-macros`, and the `*-sys` crates provide engine bindings, native/runtime utilities, artifact fetching, and supporting infrastructure.

## Repo Layout

- `slab-server`: thin HTTP gateway; exposes `/v1` routes via axum. Business logic lives in `slab-app-core`.
- `slab-app-core`: HTTP-free business logic (domain, infra, context, config). Use `tauri` feature for native IPC via `slab_app_core::tauri_bridge::register()`. Migrations are in `slab-app-core/migrations/`.
- `slab-runtime`: gRPC server and runtime worker entry points.
- `slab-runtime-core` (directory `slab-core/`): runtime library only; keep HTTP and SQL concerns out.
- `slab-agent`: agent orchestration library and tool router abstractions.
- `slab-types`: shared semantic types, settings models, load specs, and other reusable Rust contracts.
- `slab-proto`: protobuf definitions and generated conversion helpers for server/runtime IPC.
- `slab-app/src`: React frontend pages for chat, image, audio, video, hub, plugins, task, setup, settings, and about.
- `slab-app/src/pages/chat`: Ant Design X chat UI and page-local wrappers.
- `slab-app/src/pages/plugins`: desktop-only plugin center, wasm function bridge, and plugin event viewport UI.
- `slab-app/src/lib/plugin-sdk.ts`: frontend bridge to Tauri plugin commands and events.
- `slab-app/src/components/ui`: shared UI primitives.
- `slab-app/src-tauri`: Tauri host, sidecar startup, plugin runtime, capabilities, permissions, and security boundaries.
- `plugins`: local runtime plugin packages loaded by the Tauri host from `plugins/<plugin-id>/`.
- `manifests`: JSON schemas and manifest assets used by settings/model tooling.
- `vendor`: vendored runtime artifacts and external resources kept in-repo.
- `testdata`: sample media, fixture models, and integration assets.

## Working Rules

- Keep inference behind `slab-server -> GrpcGateway -> slab-runtime -> slab-runtime-core`; the desktop app can talk to the sidecar gateway via HTTP **or** call `slab-app-core` directly via native Tauri IPC.
- Extend the existing `/v1/*` API modules instead of adding a parallel API tree. The current surface includes `agent`, `audio`, `backend`, `chat`, `ffmpeg`, `images`, `models`, `session`, `settings`, `setup`, `system`, `tasks`, and `video`.
- Keep long-running AI work in task-oriented flows when the feature already follows that model.
- Prefer `slab-types` and `slab-proto` for contracts that need to cross crate boundaries instead of duplicating shapes.
- Keep `slab-agent` pure: storage, HTTP, SSE/WebSocket, and model adapters belong in `slab-server`, behind the port traits.
- Preserve Tauri CSP, capabilities, permissions, sidecar boundaries, and the `plugins/<plugin-id>/plugin.json` contract unless the task explicitly requires a change.
- `plugins/` is runtime plugin content, not AI skill content; `.agents/skills` is only for agent guidance.
- SQLx migrations in `slab-app-core/migrations/` are append-only.
- Cargo excludes `slab-app/src`, so Rust tooling does not validate the TypeScript frontend.
- When backend API shapes change, regenerate `slab-app/src/lib/api/v1.d.ts` from `http://localhost:3000/api-docs/openapi.json`.

## Build and Test

From the repo root:

```sh
cargo build --workspace
cargo test --workspace
cargo check --workspace
cargo check -p slab-server
cargo check -p slab-runtime
cargo check -p slab-app
cargo make check
cargo make test
cargo make dev
```

Frontend and Tauri:

```sh
cd slab-app
bun install
bun run dev
bun run build
bun run tauri dev
bun run tauri build
bun run api
bun run color:oklch -- background=#f7f9fb primary=#0d9488
```

Server compatibility tests:

```sh
python -m pip install -r slab-server/tests/requirements.txt
pytest slab-server/tests
```

## AI Docs Maintenance

- Keep `AGENTS.md`, `CLAUDE.md`, `.github/copilot-instructions.md`, and `docs/ai-skill-map.md` aligned when the skill list, workflow, architecture snapshot, plugin/runtime boundaries, or build commands change.
- Do not document repo-local skills that do not exist on disk.
- When adding or removing workspace members, plugin surfaces, or desktop sidecar behavior, update this doc set in the same change.
