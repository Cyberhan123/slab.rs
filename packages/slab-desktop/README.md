# @slab/desktop

React frontend application for the Slab desktop shell.

## Role

`@slab/desktop` is the primary frontend package for the Slab Tauri application. It provides the full user interface, including:

- Chat interface with streaming language model responses (Ant Design X).
- Image generation, audio transcription, and video processing pages.
- Model hub, plugin center, task queue, and settings views.
- Integration with `bin/slab-server` via `openapi-fetch` and TanStack Query.
- Workspace Monaco language features via `monaco-languageclient` over `bin/slab-server` WebSocket LSP sessions.
- Integration with host-only Tauri commands via `@tauri-apps/api` for plugin/runtime shell features.
- Theme and layout using `@slab/components` (shared shadcn/ui + Tailwind 4 primitives).
- Internationalization via `@slab/i18n`.

## Stack

- React 19, Vite, React Router 7
- Ant Design X, Tailwind CSS 4, Radix UI
- TanStack Query, `openapi-fetch`, `openapi-react-query`
- Zustand (client state), i18next (i18n)
- TypeScript

## Type

Bun-managed frontend package.

## Testing

- `src/**/__tests__/*.test.ts[x]`: pure unit tests for hooks, stores, and logic helpers.
- `tests/browser/visual/*.browser.test.tsx`: browser-mode page tests that use `page` from `vitest/browser` and `toMatchScreenshot` for visual regression.
- `tests/browser/e2e/*.browser.test.tsx`: browser-mode component/page flows backed by mocked APIs. These stay under `bun run test:browser`.
- `tests/e2e/**/*.test.ts`: fullstack E2E tests. Run only from the repository root with `bun run test:e2e`; the shared harness starts `bun run dev`, waits for the desktop UI and `/health`, and kills its dev process tree after the run.
- `tests/manual/*.test.ts`: opt-in diagnostics for real local-dev environments. These are not part of the root E2E command.
- Browser visual screenshots are platform-scoped by Vitest (`*-chromium-<platform>.png`). Keep baselines per platform and refresh them with `bun run test:browser:update` on the platform that produced the drift; do not copy a baseline across OS families.
- Run unit tests with `bun run test:run`.
- Run page/browser regression tests with `bun run test:browser`.
- Refresh desktop screenshot baselines with `bun run test:browser:update`.
- Run fullstack E2E tests with root `bun run test:e2e`.

## License

AGPL-3.0-only. See the root [LICENSE](../../LICENSE).
