import { defineProject, mergeConfig } from "vitest/config";

import { vitestBase } from "../../vitest.base.ts";

export default defineProject(
  mergeConfig(vitestBase, {
    test: {
      name: "core",
      environment: "node",
      include: ["src/**/*.test.ts"],
    },
  }),
);
