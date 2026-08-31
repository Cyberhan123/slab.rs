# @slab/ui

Feature UI layer for the Slab frontends (the "ui" of the core/ui/shell DDD split).

## Role

All pages, shared components, hooks, layouts, routes, and the app assembly that both shells mount.

- Pages under `src/pages/*`: assistant, workspace, settings, hub, image, video, audio, task, plugins, setup, about.
- `src/app`: `App.tsx` / `web-app.tsx` / `app-guards.tsx` app assembly. `src/routes/route-meta.ts` is the static source of truth for route metadata (header/sidebar), consumed by `src/hooks/use-header.ts`.
- The workspace Monaco editor and `monaco-languageclient` LSP sessions over `bin/slab-server` WebSockets live here.

## Stack

- React 19, React Router 7, `@ai-sdk/react` (`useChat`), monaco-vscode-api family
- `@slab/components`, `@slab/i18n`, `@slab/core` ports/stores/harness client

## Type

Bun-managed frontend package, consumed as workspace source.

## Validation

- `bun run build` (tsc --noEmit), `bun run lint` / `bun run lint:fix`, `bun run test:run`, `bun run test:browser` (browser mode)
- Covered by root `bun run check:frontend`; broader test gates run from the repo root (`bun run test:frontend`, `bun run test:browser`).

## Hard Boundaries

- No platform adapters or port implementations — those belong to `@slab/core` (`src/infra/*`). UI reaches platform capabilities only through core ports.
- No shell bootstrap — shells stay in `packages/slab-desktop` and `packages/slab-web`.
- Route registration and header/sidebar metadata come from route objects; do not reintroduce parallel header-meta or router-wrapper layers.

## License

AGPL-3.0-only. See the root [LICENSE](../../LICENSE).
