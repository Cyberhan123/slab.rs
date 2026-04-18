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
    "packages/slab-desktop/src/lib/api/v1.d.ts",
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
    "jsx-a11y/prefer-tag-over-role": "warn",
    "jsx-a11y/anchor-has-content": "warn",
    "jsx-a11y/no-redundant-roles": "warn",
  },
  overrides: [
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
      ],
      env: {
        vitest: true,
      },
    },
  ],
});
