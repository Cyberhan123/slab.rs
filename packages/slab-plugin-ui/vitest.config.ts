import path from "node:path";

import { defineProject, mergeConfig } from "vitest/config";

import { vitestBase } from "../../vitest.base";

export default defineProject(
  mergeConfig(vitestBase, {
    test: {
      name: "plugin-ui",
      environment: "node",
      include: ["src/**/*.test.ts"],
    },
    resolve: {
      alias: {
        // @slab/components ships source (exports "./*" -> "./src/*.tsx") and its
        // component files import "@/lib/utils" internally. Any consumer that
        // resolves @slab/components at source must map "@" at the components
        // source root, otherwise those internal imports fail to resolve.
        "@": path.resolve(__dirname, "../slab-components/src"),
      },
    },
  }),
);
