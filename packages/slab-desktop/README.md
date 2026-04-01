# @slab/desktop

React frontend application for the Slab desktop shell.

## Role

`@slab/desktop` is the primary frontend package for the Slab Tauri application. It provides the full user interface, including:

- Chat interface with streaming language model responses (Ant Design X).
- Image generation, audio transcription, and video processing pages.
- Model hub, plugin center, task queue, and settings views.
- Integration with `bin/slab-server` via `openapi-fetch` and TanStack Query.
- Integration with native Tauri IPC commands via `@tauri-apps/api`.
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

## License

AGPL-3.0-only. See the root [LICENSE](../../LICENSE).
