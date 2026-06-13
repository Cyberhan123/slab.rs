import { fileURLToPath } from "node:url"

import { defineConfig } from "vitest/config"

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  test: {
    name: "desktop-assistant-e2e-fuzzing",
    include: ["tests/e2e/**/*.fuzzing.test.ts"],
    environment: "node",
    fileParallelism: false,
    hookTimeout: 120_000,
    testTimeout: 900_000,
    teardownTimeout: 60_000,
  },
})
