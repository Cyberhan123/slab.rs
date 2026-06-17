export const frontendVitestProjects = [
  "packages/api/vitest.config.ts",
  "packages/slab-i18n/vitest.config.ts",
  "packages/slab-plugin-sdk/vitest.config.ts",
  "packages/slab-plugin-cli/vitest.config.ts",
  "packages/slab-plugin-ui/vitest.config.ts",
  "packages/slab-desktop/vitest.config.ts",
  "packages/vitest-rust-reporter/vitest.unit.config.ts",
] as const;

export const allVitestProjects = [
  ...frontendVitestProjects,
  "packages/slab-desktop/vitest.browser.config.ts",
  "packages/slab-components/vitest.config.ts",
  "packages/vitest-rust-reporter/vitest.config.ts",
  "bin/slab-server/tests/vitest.config.ts",
] as const;
