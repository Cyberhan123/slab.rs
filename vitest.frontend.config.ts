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
      ],
      thresholds: {
        lines: 85,
        functions: 82,
        branches: 74,
        statements: 85,
      },
    },
  },
});
