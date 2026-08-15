import { defineConfig } from "vitest/config";

import { frontendVitestProjects } from "./vitest.projects";

export default defineConfig({
  test: {
    projects: [...frontendVitestProjects],
    reporters: ["default"],
    coverage: {
      provider: "v8",
      reporter: ["text", "json", "html"],
      exclude: [
        "node_modules/",
        "**/vitest.setup.ts",
        "**/*.config.*",
        "**/*.browser.test.*",
        "**/dist/**",
        "**/tests/browser/**",
        "**/e2e/**",
        "packages/vitest-rust-reporter/**",
        "packages/slab-test-utils/**",
        "packages/slab-desktop/src/main.tsx",
        "packages/slab-ui/src/main.tsx",
        "packages/slab-ui/src/app/**",
        "packages/slab-components/src/index.ts",
        "packages/slab-plugin-ui/src/index.ts",
        "packages/slab-plugin-cli/src/index.ts",
        "packages/slab-ui/src/components/error-boundary.tsx",
        "packages/slab-core/src/harness/testing/**",
      ],
      thresholds: {
        lines: 74,
        functions: 68,
        branches: 69,
        statements: 75,
      },
    },
  },
});
