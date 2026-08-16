import { fileURLToPath } from "node:url"

import { defineConfig } from "vitest/config"

// Serial tail for workspace.test.ts: it opens/migrates the SHARED server's
// global workspace, which cannot run alongside the parallel agent/apply-patch
// suite (their tools resolve against the workspace root) and is a known flake.
// Boils its own shared server via the same globalSetup; runs strictly serially.
export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  test: {
    name: "desktop-e2e-workspace",
    include: ["tests/e2e/workspace.test.ts"],
    environment: "node",
    globalSetup: [fileURLToPath(new URL("./tests/e2e/support/e2e-global-setup.ts", import.meta.url))],
    fileParallelism: false,
    hookTimeout: 900_000,
    testTimeout: 900_000,
    teardownTimeout: 90_000,
  },
})
