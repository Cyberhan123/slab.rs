import { fileURLToPath } from "node:url"

import { defineConfig } from "vitest/config"

// Scripted-LMM memory phase2 consolidation suite: a dedicated slab-server +
// Vite stack booted with SLAB_E2E_MODE=1 AND the agent memory pipeline
// enabled (per-run memory root), seeded with one succeeded phase1 output row
// so the first root agent start spawns the `Consolidate memory workspace (…)`
// system subagent. A stopped phase2 run keeps the old watermark, so every
// later root turn may re-spawn the task — this stack can NEVER be shared with
// another suite, mirroring the subagent config's strictly-serial shape.
export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  test: {
    name: "desktop-e2e-memory",
    include: ["tests/e2e/memory-scripted.test.ts"],
    environment: "node",
    globalSetup: [fileURLToPath(new URL("./tests/e2e/support/e2e-memory-global-setup.ts", import.meta.url))],
    setupFiles: [
      fileURLToPath(new URL("./tests/e2e/support/e2e-failure-diagnostics.ts", import.meta.url)),
    ],
    fileParallelism: false,
    hookTimeout: 300_000,
    testTimeout: 240_000,
    teardownTimeout: 90_000,
  },
})
