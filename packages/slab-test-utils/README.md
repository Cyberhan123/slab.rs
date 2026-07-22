# @slab/test-utils

Shared vitest test infrastructure for slab frontend packages. Source-only workspace package (no build, no `dist/`) — consumed directly as TypeScript source through each package's vitest resolve alias (e.g. `desktopVitestResolve` in `packages/slab-desktop/vitest.shared.ts`) or the `exports` map.

## Role

Centralizes the cross-cutting test boilerplate that was duplicated across `slab-desktop` tests:

- **Mock factories** (`setupSlabI18nMock`, `setupToastMock`, `setupApiMock`) — each call returns a fresh module shape with `vi.fn()` handles. Tests wire them as `vi.mock(path, () => setupXxxMock())` and read handles back through re-import + `vi.mocked()`.
- **monaco-vscode deep-path stubs** (`monacoUriStub`, `monacoEventStub`, `monacoFilesServiceOverrideStub` + the `MONACO_*_PATH` constants) — centralize the version-coupled `@codingame/monaco-vscode-*` internals so they are not inlined per test.
- **`renderWithProviders`** — `@testing-library/react` `render` wrapped with a no-retry `QueryClient` (queries + mutations) and an optional `MemoryRouter`. Access via the `@slab/test-utils/providers/render-with-providers` subpath (kept out of the root barrel so non-React consumers do not pull React).
- **`defineFixture`** — typed default + `Partial<T>` override builder mirroring the existing `createBackend(overrides)` / `fileEntry(overrides)` idiom.
- **`setup/jsdom`** — jsdom global setup (jest-dom matchers, `afterEach(cleanup)`, IntersectionObserver / ResizeObserver / matchMedia stubs), migrated verbatim from `packages/slab-desktop/vitest.setup.ts`.

## Local commands

Lint and tests are run from the repo root:

- `bun run lint` — oxlint over all frontend packages (this package is included in the root `lint` script).
- This package has no own test suite (its `test` script is a no-op); it is exercised by its consumers' vitest projects.

## Hard boundaries

- **Never import this package from production (non-test) code.** It is test-only infrastructure.
- **`@slab/test-utils/setup/jsdom` must never be added to a browser project's `setupFiles`.** Its jsdom globals (IntersectionObserver / ResizeObserver / matchMedia) would shadow the real browser APIs. Browser projects keep their own `tests/browser/vitest.setup.ts`.
- Mock factories must be **called inside the consuming test's `vi.mock` / `vi.hoisted`**, never at this package's module top level — vitest hoists `vi.mock` calls, and only the test file's own `vi.mock` registrations are reliably ordered before the mocked module's import.
