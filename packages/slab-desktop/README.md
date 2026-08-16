# @slab/desktop

React frontend application for the Slab desktop shell.

## Role

`@slab/desktop` is the pure Tauri shell: it assembles the desktop platform (module-level seams + `SlabProvider` with `createDesktopPorts()`), mounts the shared routes from `@slab/ui`, and owns the desktop-only test suites. All pages, stores, hooks, layouts, and routes live in `@slab/ui`; ports/seams/infra adapters live in `@slab/core`.

- Installs Tauri platform adapters into `@slab/core` seams at startup (`assembleDesktopPlatform`).
- Mounts `SlabProvider` + `QueryClientProvider` around `RouterProvider` (`src/main.tsx`).
- Workspace Monaco language features run inside `@slab/ui` via `monaco-languageclient` over `bin/slab-server` WebSocket LSP sessions.

## Stack

- React 19, Vite, React Router 7
- `@slab/ui` (all features), `@slab/core` (ports/seams), `@slab/components`, `@slab/i18n`
- TanStack Query
- TypeScript

## Dependency anchors

The shell's `package.json` intentionally declares only what its own sources (and tests/vite config) resolve. Two exceptions are **vite dev anchors**, kept for tooling resolution even though the real consumer is `@slab/ui` source code:

- `vscode` — `vite.config.ts` aliases the bare `vscode` specifier to `./node_modules/vscode` (bun does not hoist this package to the workspace root).
- `@codingame/monaco-vscode-api` — referenced as a bare id in `optimizeDeps.include`.

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

## Plan F Guardrails

- PR CI keeps the fast desktop gate in `bun run check:frontend`, `bun run test:frontend`, `bun run test:browser`, contract drift checks, schema drift checks, and `bun run check:bundle-budget`.
- Fullstack release-risk flows stay in `bun run test:e2e` and are run from release, manual, or nightly CI paths.
- Desktop bundle budgets are tracked in `bundle-budget.json`; run `bun run build:desktop` before `bun run check:bundle-budget`.
- Runtime rollback flags are exposed through settings PMIDs:
  - `guardrails.assistant_sse_resume`
  - `guardrails.workspace_monaco_lazy`
  - `guardrails.assistant_error_envelope_rendering`

## License

AGPL-3.0-only. See the root [LICENSE](../../LICENSE).
