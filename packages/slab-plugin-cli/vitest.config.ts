import { defineProject, mergeConfig } from "vitest/config";

import { vitestBase } from "../../vitest.base";

export default defineProject(
  mergeConfig(vitestBase, {
    test: {
      name: "plugin-cli",
      environment: "node",
      include: ["tests/**/*.test.ts"],
    },
  }),
);
