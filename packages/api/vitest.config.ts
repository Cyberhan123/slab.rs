import { defineProject, mergeConfig } from "vitest/config";

import { vitestBase } from "../../vitest.base";

export default defineProject(
  mergeConfig(vitestBase, {
    test: {
      name: "api",
      environment: "jsdom",
      include: ["src/**/*.test.ts"],
    },
  }),
);
