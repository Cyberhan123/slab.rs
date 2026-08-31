# @slab/web

Browser shell for the Slab frontends (the web "shell" of the core/ui/shell DDD split).

## Role

Thin Vite shell that mounts the shared `@slab/ui` app in the browser against a standalone `slab-server`.

- `src/main.tsx` + `src/web-app.tsx` bootstrap; `src/health-status.tsx` server health surface.
- Server location is configured via `.env.local` (copy `.env.example`): `VITE_API_BASE_URL` for a direct API base, or `VITE_API_PROXY_TARGET` to proxy `/v1` through the Vite dev server.
- Pair with root `bun run dev:server`; run the shell itself with `bun run dev`.

## Stack

- React 19, Vite
- All features come from `@slab/ui`, `@slab/core`, and `@slab/api`; this package adds no feature code

## Type

Bun-managed frontend package (Vite app).

## Validation

- `bun run lint`, `bun run build` (tsc --noEmit + vite build), `bun run preview`
- No dedicated test suite; the shell build is part of root `bun run check:frontend` (runs last, after the packages it depends on).

## Hard Boundaries

- No feature pages, components, or domain logic, and no growing shell-specific state; feature work belongs in `@slab/ui` / `@slab/core`.
- Browser-only: no Tauri APIs.

## License

AGPL-3.0-only. See the root [LICENSE](../../LICENSE).
