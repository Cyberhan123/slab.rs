import { defineProject, mergeConfig } from "vitest/config";

import { vitestBase } from "../../vitest.base.ts";

export default defineProject(
  mergeConfig(vitestBase, {
    test: {
      name: "api",
      environment: "jsdom",
      include: ["src/**/*.test.ts"],
    },
  }),
);
