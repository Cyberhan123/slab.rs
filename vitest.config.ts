import { defineConfig } from "vitest/config";

import { allVitestProjects } from "./vitest.projects";

export default defineConfig({
  test: {
    projects: [...allVitestProjects],
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
    },
  },
});
