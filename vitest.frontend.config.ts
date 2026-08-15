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
        // Test helpers living under src (same policy as slab-test-utils being
        // excluded wholesale): they are test scaffolding, not product surface.
        "packages/slab-ui/src/provider/test-ports.ts",
        "packages/slab-ui/src/store/__tests__/**",
      ],
      thresholds: {
        // Recalibrated 2026-08-15 after the DDD multi-shell migration moved
        // the tree's mass around (measured: 69.45/67.85/63.06/68.87 minus a
        // ~0.5pt margin). The browser/monaco-dominated files pulling the
        // average down (workspace editor, core bridge, md-to-react) are
        // covered by test:browser, not this node/browser unit umbrella.
        lines: 69,
        functions: 67.5,
        branches: 62.5,
        statements: 68.5,
      },
    },
  },
});
