import { defineConfig } from "vitest/config";

// Opt-in real-environment diagnostics (see README "Testing"): the files under
// tests/manual expect a live dev server (and possibly downloaded model
// weights) and gate themselves on env vars, so they are not part of any
// default gate. Run one explicitly from the repo root, e.g.:
//
//   SLAB_E2E_REAL_MODEL=1 bunx vitest run \
//     --config packages/slab-desktop/vitest.manual.config.ts \
//     assistant.real-dev-qwen35-thinking
export default defineConfig({
  test: {
    // Pin the root to this package so the config works from any cwd.
    root: import.meta.dirname,
    name: "desktop-manual",
    environment: "node",
    include: ["tests/manual/*.test.ts"],
    // Real-model loads (multi-GB weights, GPU prefill) can take minutes.
    testTimeout: 15 * 60_000,
    hookTimeout: 60_000,
  },
});
