# @slab/core

Cross-shell domain layer for the Slab frontends (the "core" of the core/ui/shell DDD split).

## Role

`@slab/core` owns everything the desktop and web shells share below the UI: the port/seam abstraction, platform adapters, the agent harness client, and domain state.

- `src/ports` + `src/platform`: seam interfaces and their assembly. Shells install platform adapters at startup; Tauri-backed adapters live in `src/infra/tauri`, browser adapters in `src/infra/web`.
- `src/harness`: the shared harness client — `harness-transport.ts` (AI SDK `useChat` transport over the harness WS surface), `conversation-controller.ts`, `turn-input.ts`, `turn-items.ts`, plus testing helpers.
- Domain areas: `src/api` client wiring, `src/models`, `src/workspace`, `src/media`, `src/system`, `src/ui-state` stores.

## Stack

- TypeScript, TanStack Query, AI SDK (`ai`), `@slab/api`, `@slab/plugin-sdk`
- Tauri API access is allowed only inside the tauri infra adapters, behind ports

## Type

Bun-managed frontend package, consumed as workspace source (`exports` map straight into `src/`).

## Validation

- `bun run build` (tsc --noEmit), `bun run lint` / `bun run lint:fix`, `bun run test:run`
- Covered by root `bun run check:frontend`; broader test gates run from the repo root (`bun run test:frontend`).

## Hard Boundaries

- No pages, components, or layouts — feature UI belongs to `@slab/ui`.
- No shell bootstrap (`main.tsx`, `index.html`, Vite app config) — shells stay in `packages/slab-desktop` and `packages/slab-web`.
- Platform-specific code goes through `src/ports` seams; do not leak Tauri or browser globals into domain modules.

## License

AGPL-3.0-only. See the root [LICENSE](../../LICENSE).
