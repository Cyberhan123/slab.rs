import { fileURLToPath } from "node:url"

import { defineConfig } from "vitest/config"

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  test: {
    name: "desktop-assistant-fullstack",
    include: ["tests/e2e/**/*.fullstack.test.ts"],
    environment: "node",
    fileParallelism: false,
    hookTimeout: 180_000,
    testTimeout: 240_000,
    teardownTimeout: 30_000,
  },
})
