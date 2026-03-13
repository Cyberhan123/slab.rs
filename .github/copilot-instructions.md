Read [../AGENTS.md](../AGENTS.md) before making changes.

Key repo facts:

- `.agents/skills` contains operational skills and should not be treated as generic source code.
- Prefer the local slab.rs skills first:
  - `slab-frontend-page`
  - `slab-ui-primitives`
  - `slab-tauri-app`
  - `slab-server-feature`
  - `slab-runtime-async`
  - `slab-ui-review`
- Use generic imported skills only as supporting references through those local skills.
- `slab-server` follows the existing `config`, `context`, `api`, `domain`, and `infra` layout.
- AI inference must stay behind the supervisor plus gRPC runtime boundary: `slab-server -> GrpcGateway -> slab-runtime -> slab-core`.
- `slab-app` is a React 19 + Vite + React Router 7 + Tauri 2 desktop app, not a single invoke-only shell.
- Frontend server state uses TanStack Query with `openapi-fetch` and `openapi-react-query`.
- Frontend client state uses Zustand.
- AI-focused frontend components use Ant Design X and `antd`, with shared Tailwind 4 primitives under `slab-app/src/components/ui`.
- Tauri security settings are explicit in `slab-app/src-tauri/tauri.conf.json`; preserve CSP and capabilities unless the task requires a deliberate change.
- If documentation and code disagree, trust the code and update the documentation.
