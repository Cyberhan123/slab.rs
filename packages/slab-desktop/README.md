# @slab/desktop

React frontend application for the Slab desktop shell.

## Role

`@slab/desktop` is the primary frontend package for the Slab Tauri application. It provides the full user interface, including:

- Chat interface with streaming language model responses (Ant Design X).
- Image generation, audio transcription, and video processing pages.
- Model hub, plugin center, task queue, and settings views.
- Integration with `bin/slab-server` via `openapi-fetch` and TanStack Query.
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
- Run unit tests with `bun run test:run`.
- Run page/browser regression tests with `bun run test:browser`.
- Refresh desktop screenshot baselines with `bun run test:browser:update`.

## License

AGPL-3.0-only. See the root [LICENSE](../../LICENSE).
