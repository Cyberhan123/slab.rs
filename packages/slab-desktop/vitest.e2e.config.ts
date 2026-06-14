import { fileURLToPath } from "node:url"

import { defineConfig } from "vitest/config"

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  test: {
    name: "desktop-e2e",
    include: ["tests/e2e/**/*.test.ts"],
    environment: "node",
    fileParallelism: false,
    hookTimeout: 900_000,
    testTimeout: 900_000,
    teardownTimeout: 90_000,
  },
})
