import { fileURLToPath } from "node:url"

import { defineConfig } from "vitest/config"

// Scripted-LLM questionnaire suite: a second, fully-owned slab-server + Vite
// stack booted with SLAB_E2E_MODE=1 (deterministic prompt-keyed LlmPort
// responses, debug builds only) and NO model. Mirrors the subagent scripted
// config shape: own globalSetup, strictly serial.
export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  test: {
    name: "desktop-e2e-questionnaire",
    include: ["tests/e2e/questionnaire-scripted.test.ts"],
    environment: "node",
    globalSetup: [
      fileURLToPath(new URL("./tests/e2e/support/e2e-questionnaire-global-setup.ts", import.meta.url)),
    ],
    setupFiles: [
      fileURLToPath(new URL("./tests/e2e/support/e2e-failure-diagnostics.ts", import.meta.url)),
    ],
    fileParallelism: false,
    hookTimeout: 300_000,
    testTimeout: 180_000,
    teardownTimeout: 90_000,
  },
})
