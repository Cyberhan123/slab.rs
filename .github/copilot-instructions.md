Read [../AGENTS.md](../AGENTS.md) before making changes.

Key repo facts:

- `.agents/skills` contains optional operational skills. `plugins/` is the desktop runtime plugin workspace and should not be treated as skill content.
- Do not assume a local slab.rs skill wrapper layer exists. Use the codebase directly unless the task clearly matches one of the real local skills:
  - `use-x-chat`
  - `x-request`
  - `x-markdown`
  - `x-chat-provider`
  - `shadcn-ui`
  - `tauri-v2`
- `slab-app` is a React 19 + Vite + React Router 7 + Tauri 2 desktop app that launches `slab-server` as a sidecar and mounts local plugin webviews. It also embeds `slab-app-core` (with the `tauri` feature) for native IPC commands.
- Frontend server state uses TanStack Query with `openapi-fetch` and `openapi-react-query`.
- Frontend client state uses Zustand.
- AI-focused frontend components use Ant Design X, with shared Tailwind 4 primitives under `slab-app/src/components/ui`.
- `slab-app-core` (directory: `slab-app-core/`) holds the HTTP-free business logic: `context/`, `domain/`, `infra/`, `config`, `model_auto_unload`. Use the `tauri` feature for `slab_app_core::tauri_bridge::register()`. Migrations live in `slab-app-core/migrations/`.
- `slab-server` is the thin HTTP (axum) gateway. It depends on `slab-app-core` for domain/infra logic; `state_extractors.rs` provides axum `FromRef` impls, and `error.rs` provides `ServerError` → HTTP response conversion. The server exposes `/v1` plus `/api-docs/openapi.json`.
- AI inference must stay behind the supervisor plus runtime boundary: `slab-server -> GrpcGateway -> slab-runtime -> slab-runtime-core`.
- `slab-runtime` supports TCP or IPC transport for llama, whisper, and diffusion workers.
- `slab-runtime-core` (directory: `slab-core/`) holds runtime orchestration, scheduler, and engine adapters. Keep HTTP and SQL concerns out.
- `slab-types` and `slab-proto` are the shared Rust contract crates for semantic types, settings/runtime models, and server/runtime IPC.
- Tauri security settings are explicit in `slab-app/src-tauri/tauri.conf.json`; preserve CSP, permissions, capabilities, and plugin boundaries unless the task requires a deliberate change.
- If documentation and code disagree, trust the code and update the documentation.
