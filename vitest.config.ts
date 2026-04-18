import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    projects: [
      "packages/slab-desktop/vitest.config.ts",
      "bin/slab-server/tests/vitest.config.ts",
    ],
    reporters: ["default"],
    coverage: {
      provider: "v8",
      reporter: ["text", "json", "html"],
      exclude: [
        "node_modules/",
        "**/vitest.setup.ts",
        "**/*.config.*",
        "**/dist/**",
        "**/e2e/**",
      ],
    },
  },
});
