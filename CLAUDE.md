# CLAUDE.md

Read [AGENTS.md](AGENTS.md) before making changes. This file only highlights the repo-specific rules Claude Code should keep in short-term memory.

## Quick Rules

- `.agents/skills` contains operational guidance, not generic source code.
- Use the slab.rs local skill layer first:
  - `slab-frontend-page`
  - `slab-ui-primitives`
  - `slab-tauri-app`
  - `slab-server-feature`
  - `slab-runtime-async`
  - `slab-ui-review`
- Use generic imported skills through those project skills rather than treating them as the default starting point.
- Current frontend stack: React 19, Vite, React Router 7, Tauri 2, TanStack Query, `openapi-fetch`, Zustand, Ant Design X, Tailwind 4, and shared `src/components/ui` primitives.
- Current server layering: `config`, `context`, `api`, `domain`, and `infra`.
- AI inference should flow through `slab-server -> GrpcGateway -> slab-runtime -> slab-core`.
- Tauri CSP is explicitly configured in `slab-app/src-tauri/tauri.conf.json`; it is not `null`.
- If repo docs and code disagree, follow the code and update the docs.
