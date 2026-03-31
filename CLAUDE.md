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
- `slab-app` starts `slab-server` as a sidecar and hosts local plugins from `plugins/`. It also embeds `slab-app-core` (`tauri` feature) for native IPC commands.
- `slab-app-core` (directory: `slab-app-core/`) is the HTTP-free business logic library. Contains `context/`, `domain/`, `infra/`, `config`, `model_auto_unload`. Enable feature `tauri` to get `slab_app_core::tauri_bridge::register()`. Migrations are in `slab-app-core/migrations/`.
- `slab-server` is the thin HTTP gateway (axum). It depends on `slab-app-core` for all domain logic; adds axum `FromRef` extractors in `state_extractors.rs` and `ServerError` → HTTP response conversion.
- `slab-runtime` serves gRPC over TCP or IPC and can enable llama, whisper, and diffusion backends independently.
- `slab-runtime-core` (directory: `slab-core/`) is runtime/orchestration only; shared contracts belong in `slab-types` and `slab-proto`.
- Preserve the current Tauri CSP, permissions, capabilities, and plugin host boundaries unless the task explicitly requires a change.
- If repo docs and code disagree, follow the code and update the docs.
