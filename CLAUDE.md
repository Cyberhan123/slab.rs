# CLAUDE.md

Read [AGENTS.md](AGENTS.md) before making changes. This file only keeps the repo facts that are worth holding in short-term memory.

## Quick Rules

- `.agents/skills` contains optional task guidance, not generic source code.
- Do not assume a project-specific skill wrapper layer exists. Use the codebase directly unless a task clearly matches one of the real local skills:
  - `use-x-chat`
  - `x-request`
  - `x-markdown`
  - `x-chat-provider`
  - `shadcn-ui`
  - `tauri-v2`
- Current frontend stack: React 19, Vite, React Router 7, Tauri 2, TanStack Query, `openapi-fetch`, Zustand, Ant Design X, and Tailwind 4.
- `slab-server` keeps the existing `config`, `context`, `api`, `domain`, and `infra` layout.
- `slab-types` is the shared Rust contract crate for semantic types and schema-friendly definitions used across crates.
- AI inference must stay behind `slab-server -> GrpcGateway -> slab-runtime -> slab-core`.
- Preserve the current Tauri CSP and capability boundaries unless the task explicitly requires a change.
- If repo docs and code disagree, follow the code and update the docs.
