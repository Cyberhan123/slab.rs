Read [../AGENTS.md](../AGENTS.md) before making changes.

Key repo facts:

- `.agents/skills` contains optional operational skills and should not be treated as generic source code.
- Do not assume a local slab.rs skill wrapper layer exists. Use the codebase directly unless the task clearly matches one of the real local skills:
  - `use-x-chat`
  - `x-request`
  - `x-markdown`
  - `x-chat-provider`
  - `shadcn-ui`
  - `tauri-v2`
- `slab-server` follows the existing `config`, `context`, `api`, `domain`, and `infra` layout.
- `slab-types` is the shared Rust contract crate for semantic types and schema-oriented definitions reused across crates.
- AI inference must stay behind the supervisor plus gRPC runtime boundary: `slab-server -> GrpcGateway -> slab-runtime -> slab-core`.
- `slab-app` is a React 19 + Vite + React Router 7 + Tauri 2 desktop app.
- Frontend server state uses TanStack Query with `openapi-fetch` and `openapi-react-query`.
- Frontend client state uses Zustand.
- AI-focused frontend components use Ant Design X, with shared Tailwind 4 primitives under `slab-app/src/components/ui`.
- Tauri security settings are explicit in `slab-app/src-tauri/tauri.conf.json`; preserve CSP and capabilities unless the task requires a deliberate change.
- If documentation and code disagree, trust the code and update the documentation.
