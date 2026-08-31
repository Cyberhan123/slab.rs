import { defineConfig } from "oxlint";

export default defineConfig({
  categories: {
    correctness: "error",
    suspicious: "warn",
    perf: "warn",
  },
  plugins: [
    "eslint",
    "oxc",
    "typescript",
    "unicorn",
    "react",
    "vitest",
    "jsx-a11y",
  ],
  env: {
    builtin: true,
    browser: true,
    node: true,
  },
  ignorePatterns: [
    "**/node_modules/**",
    "**/dist/**",
    "**/coverage/**",
    "**/.vitepress/**",
    "packages/api/src/v1.d.ts",
    "packages/slab-plugin-sdk/browser/**",
    // Reference-only UI samples — never imported or shipped, not product code.
    "docs/development/references/assistant-message-reference/**",
  ],
  settings: {
    react: {
      version: "19.2.5",
    },
  },
  rules: {
    "react/react-in-jsx-scope": "off",
    "react-hooks/exhaustive-deps": "warn",
    "vitest/require-mock-type-parameters": "warn",
    "jsx-a11y/media-has-caption": "warn",
    "jsx-a11y/click-events-have-key-events": "warn",
    "jsx-a11y/no-noninteractive-element-interactions": "warn",
    "jsx-a11y/prefer-tag-over-role": "warn",
    "jsx-a11y/anchor-has-content": "warn",
    "jsx-a11y/no-redundant-roles": "warn",
  },
  overrides: [
    {
      // DDD boundary: @slab/core must stay pure — no view layer, no UI state
      // libraries, no concrete ui imports.
      files: ["packages/slab-core/src/**/*.ts", "packages/slab-core/src/**/*.tsx"],
      rules: {
        "eslint/no-restricted-imports": [
          "error",
          {
            paths: [
              "react",
              "react-dom",
              "react-dom/client",
              "sonner",
              "@slab/ui",
              "@slab/components",
              "@tanstack/react-query",
              "zustand",
            ],
            patterns: [
              {
                group: ["react/*", "@slab/ui/*", "@slab/components/*"],
                message: "@slab/core must not import view-layer modules.",
              },
              {
                group: ["@slab/core/infra/*"],
                message:
                  "@slab/core platform seams (src/platform/*) must not reach into concrete infra adapters; shells install them instead.",
              },
            ],
          },
        ],
      },
    },
    {
      // Platform seams must stay adapter-free; only the create-ports assembly
      // factory (and the shells) may wire concrete infra adapters in.
      files: [
        "packages/slab-core/src/platform/detect.ts",
        "packages/slab-core/src/platform/image-src.ts",
        "packages/slab-core/src/platform/notifications.ts",
        "packages/slab-core/src/platform/plugin-host.ts",
      ],
      rules: {
        "eslint/no-restricted-imports": [
          "error",
          {
            patterns: [
              {
                group: ["../infra/*", "../../infra/*"],
                message:
                  "platform seams must not import concrete infra adapters (only create-ports.ts assembles them).",
              },
            ],
          },
        ],
      },
    },
    {
      // @slab/core/src/infra/tauri is the ONLY place allowed to touch Tauri.
      files: ["packages/slab-core/src/infra/tauri/**/*.ts"],
      rules: {
        "eslint/no-restricted-imports": [
          "error",
          {
            paths: [
              "react",
              "react-dom",
              "sonner",
              "@slab/ui",
              "@slab/components",
            ],
            patterns: [
              {
                group: ["react/*", "@slab/ui/*", "@slab/components/*"],
                message: "infra adapters must stay view-free.",
              },
            ],
          },
        ],
      },
    },
    {
      // DDD boundary: @slab/ui consumes ports/seams, never concrete adapters,
      // and never Tauri directly.
      files: ["packages/slab-ui/src/**/*.ts", "packages/slab-ui/src/**/*.tsx"],
      rules: {
        "eslint/no-restricted-imports": [
          "error",
          {
            patterns: [
              {
                group: ["@tauri-apps/*", "@slab/core/infra/*"],
                message:
                  "@slab/ui must use the injected ports (@slab/core platform seams / SlabProvider), not concrete infra adapters.",
              },
            ],
          },
        ],
      },
    },
    {
      files: [
        "packages/slab-desktop/**/*.test.ts",
        "packages/slab-desktop/**/*.test.tsx",
        "packages/slab-desktop/**/*.spec.ts",
        "packages/slab-desktop/**/*.spec.tsx",
        "packages/slab-desktop/tests/**/*.ts",
        "packages/slab-desktop/tests/**/*.tsx",
        "packages/slab-desktop/vitest.setup.ts",
        "bin/slab-server/tests/**/*.ts",
        "packages/vitest-rust-reporter/src/**/*.test.ts",
      ],
      env: {
        vitest: true,
      },
    },
    {
      files: ["packages/vitest-rust-reporter/src/runtime/rust.test.ts"],
      rules: {
        "jest/no-disabled-tests": "off",
        "jest/valid-title": "off",
        "vitest/no-disabled-tests": "off",
        "vitest/no-conditional-tests": "off",
        "vitest/valid-title": "off",
      },
    },
  ],
});
