import { fileURLToPath } from "node:url"

import { defineConfig } from "vitest/config"

// Scripted-LMM subagent delegation suite: a second, fully-owned slab-server +
// Vite stack booted with SLAB_E2E_MODE=1 (deterministic prompt-keyed LlmPort
// responses, debug builds only) and NO model. Mirrors the workspace serial-tail
// config shape: own globalSetup, strictly serial (several scenarios drive
// interrupts and permission-mode switches that this stack may do freely).
export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  test: {
    name: "desktop-e2e-subagent",
    include: ["tests/e2e/subagent-scripted.test.ts"],
    environment: "node",
    globalSetup: [fileURLToPath(new URL("./tests/e2e/support/e2e-subagent-global-setup.ts", import.meta.url))],
    fileParallelism: false,
    hookTimeout: 300_000,
    testTimeout: 180_000,
    teardownTimeout: 90_000,
  },
})
