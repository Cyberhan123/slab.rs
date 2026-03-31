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
- `bin/slab-app` is the Tauri 2 desktop host that launches `slab-server` as a sidecar and mounts local plugin webviews. Its frontend is `packages/slab-desktop`.
- `packages/slab-desktop` is the React 19 + Vite + React Router 7 frontend app, managed as a bun workspace package (`@slab/desktop`).
- `packages/slab-components` is the shared shadcn/ui-based React component library (`@slab/components`), with Radix UI + Tailwind 4 primitives.
- `packages/slab-i18n` is the shared i18n package (`@slab/i18n`) using i18next and react-i18next.
- Frontend server state uses TanStack Query with `openapi-fetch` and `openapi-react-query`.
- Frontend client state uses Zustand.
- AI-focused frontend components use Ant Design X, with shared Tailwind 4 primitives from `packages/slab-components/src`.
- `slab-server` follows the existing `config`, `context`, `api`, `domain`, and `infra` layout, exposes `/v1` plus `/api-docs/openapi.json`, and wires `slab-agent` through server-side adapters.
- AI inference must stay behind the supervisor plus runtime boundary: `slab-server -> GrpcGateway -> slab-runtime -> crates/slab-core`.
- `slab-runtime` supports TCP or IPC transport for llama, whisper, and diffusion workers.
- `crates/slab-types` and `crates/slab-proto` are the shared Rust contract crates for semantic types, settings/runtime models, and server/runtime IPC.
- All Rust library crates live in `crates/` (e.g., `crates/slab-core`, `crates/slab-types`, `crates/slab-agent`).
- Tauri security settings are explicit in `bin/slab-app/src-tauri/tauri.conf.json`; preserve CSP, permissions, capabilities, and plugin boundaries unless the task requires a deliberate change.
- If documentation and code disagree, trust the code and update the documentation.
