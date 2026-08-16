import { fileURLToPath } from "node:url"

import { defineConfig } from "vitest/config"

// Concurrency knob: how many e2e files run in parallel against the ONE shared
// slab-server. It MUST match the server's LLM `backend_capacity`
// (SLAB_BACKEND_CAPACITY), which startE2eRuntime sets from this same value —
// otherwise excess turns block on the runtime's 30s GPU-acquire timeout and
// fail. Default 2 (conservative for a shared 9B model whose weights load once
// and whose concurrency costs only `n_ctx` slots); raise via env, e.g.
// `SLAB_E2E_CONCURRENCY=4 bun run test:e2e`.
const concurrency = Number(process.env.SLAB_E2E_CONCURRENCY ?? 2)

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  test: {
    name: "desktop-e2e",
    include: ["tests/e2e/*.test.ts"],
    // workspace.test.ts opens/migrates the SHARED server's global workspace,
    // which would derail concurrent agent/apply-patch turns (their tools resolve
    // against the workspace root). It is also a known flake; run it on its own.
    exclude: ["**/node_modules/**", "tests/e2e/workspace.test.ts"],
    environment: "node",
    globalSetup: [fileURLToPath(new URL("./tests/e2e/support/e2e-global-setup.ts", import.meta.url))],
    fileParallelism: true,
    maxWorkers: concurrency,
    minWorkers: 1,
    hookTimeout: 900_000,
    testTimeout: 900_000,
    teardownTimeout: 90_000,
  },
})
