import { fileURLToPath } from "node:url"

import { defineConfig } from "vitest/config"

import { desktopVitestResolve } from "./vitest.shared"

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  test: {
    name: "desktop-e2e",
    environment: "node",
    fileParallelism: false,
    hookTimeout: 900_000,
    include: ["tests/e2e/desktop/**/*.test.ts"],
    testTimeout: 300_000,
  },
  resolve: desktopVitestResolve,
})
