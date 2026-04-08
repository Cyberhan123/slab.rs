# CLAUDE.md

Read [AGENTS.md](AGENTS.md) before making changes. This file only keeps the repo facts that are worth holding in short-term memory.

## Quick Rules

- `.agents/skills` contains optional task guidance. `plugins/` contains runtime plugin packages, not AI skills.
- Do not assume a project-specific skill wrapper layer exists. Use the codebase directly unless a task clearly matches one of the real local skills:
  - `use-x-chat`
  - `x-request`
  - `x-markdown`
  - `x-chat-provider`
  - `shadcn-ui`
  - `tauri-v2`
- Current frontend stack: React 19, Vite, React Router 7, Tauri 2, TanStack Query, `openapi-fetch`, `openapi-react-query`, Zustand, Ant Design X, `i18next`, and Tailwind 4.
- `bin/slab-app` is the Tauri desktop host. Its frontend is `packages/slab-desktop`, which uses `packages/slab-components` for UI and `packages/slab-i18n` for i18n. Native IPC commands live in `bin/slab-app/src-tauri/src/api`, call `crates/slab-app-core` directly, and use the shared core runtime supervisor through a Tauri sidecar adapter.
- `packages/slab-components` is the shadcn/ui-based shared component library (workspace package `@slab/components`).
- `packages/slab-i18n` is the shared i18n package (workspace package `@slab/i18n`) with i18next and react-i18next.
- `packages/slab-desktop` is the main React frontend app (workspace package `@slab/desktop`).
- Public VitePress pages live in `docs/`, internal contributor docs live in `docs/development/`, and published JSON Schemas are generated into `docs/public/manifests/v1/` with `bun run docs:schemas`.
- All Rust library crates live in `crates/` (e.g., `crates/slab-runtime-core`, `crates/slab-types`, `crates/slab-app-core`).
- Binary executables live in `bin/` (e.g., `bin/slab-server`, `bin/slab-runtime`, `bin/slab-app`).
- `crates/slab-app-core` (package: `slab-app-core`) is the HTTP-free business logic library. Contains `context/`, `domain/`, `infra/`, `config`, `model_auto_unload`, and the shared `runtime_supervisor`. Migrations are in `crates/slab-app-core/migrations/`.
- `bin/slab-server` is the thin HTTP gateway (axum) and headless host. It depends on `crates/slab-app-core` for all domain logic; adds axum `FromRef` extractors in `state_extractors.rs`, `ServerError` → HTTP response conversion, and uses the shared core runtime supervisor through a `tokio::process` adapter. Exposes `/v1` plus `/api-docs/openapi.json`.
- `bin/slab-runtime` serves gRPC over TCP or IPC and is the only runtime composition root. It now uses an in-package `api` / `application` / `domain` / `infra` layout, with gRPC bootstrap in `bin/slab-runtime/src/api/server.rs`, application orchestration in `bin/slab-runtime/src/application/services`, and flattened GGML, Candle, and ONNX backend implementations under `bin/slab-runtime/src/infra/backends`.
- `crates/slab-runtime-core` (package: `slab-runtime-core`) is the pure scheduler/backend-protocol crate; backend composition and typed runtime codecs belong in `bin/slab-runtime`, and shared contracts belong in `crates/slab-types` and `crates/slab-proto`.
- Preserve the current Tauri CSP, permissions, capabilities, and plugin host boundaries unless the task explicitly requires a change.
- If repo docs and code disagree, follow the code and update the docs.
