import { defineProject } from "vitest/config";

/**
 * Shared base for per-package vitest project configs.
 *
 * Per-package configs should be defined as:
 *   export default defineProject(mergeConfig(vitestBase, { test: { name, environment, include, ... } }))
 *
 * Conventions baked in here (deliberate choices — see slab test plan):
 * - `globals` is NOT enabled. Every test file imports
 *   `{ describe, it, expect, vi, beforeEach, ... }` from "vitest". Only the
 *   `desktop` project historically set `globals: true`; it has been removed so
 *   all projects behave identically.
 * - `restoreMocks` / `resetMocks` are NOT enabled globally. They revert
 *   `vi.spyOn` originals and would silently break the `vi.hoisted` + per-test
 *   spy patterns used across the repo. Tests that need full isolation opt in
 *   with `afterEach(() => vi.restoreAllMocks())`.
 * - `clearMocks` resets call history between tests. Note it does NOT drain
 *   `mockResolvedValueOnce` queues — audit before relying on it.
 * - Browser/e2e/rust-reporter projects keep their own configs and do NOT merge
 *   this base (they have different environment, pool, and setup concerns).
 * - `coverage` is intentionally NOT set here. Coverage (provider, reporters,
 *   exclude, thresholds) is configured centrally in the root
 *   `vitest.frontend.config.ts` / `vitest.config.ts`. Adding a partial coverage
 *   block here would shadow the root `coverage.exclude` at the project level and
 *   silently drag the aggregate below threshold.
 */
export const vitestBase = defineProject({
  test: {
    clearMocks: true,
    unstubGlobals: true,
    unstubEnvs: true,
    exclude: ["**/node_modules/**", "**/dist/**", "**/*.config.*"],
  },
});
