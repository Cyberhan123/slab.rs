import { defineProject, mergeConfig } from "vitest/config";

import { vitestBase } from "../../vitest.base";

export default defineProject(
  mergeConfig(vitestBase, {
    test: {
      name: "i18n",
      environment: "node",
      include: ["src/**/*.test.ts"],
    },
  }),
);
