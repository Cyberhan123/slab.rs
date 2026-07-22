import react from "@vitejs/plugin-react";
import { defineProject, mergeConfig } from "vitest/config";

import { desktopVitestResolve } from "./vitest.shared";
import { vitestBase } from "../../vitest.base";

export default defineProject(
  mergeConfig(vitestBase, {
    plugins: [react()],
    test: {
      name: "desktop",
      environment: "jsdom",
      setupFiles: ["./vitest.setup.ts"],
      css: true,
      // vitestBase already excludes node_modules / dist / config files; add the
      // desktop-only trees that are exercised by the browser/e2e projects.
      exclude: ["tests/browser/**", "tests/e2e/**", "tests/manual/**"],
    },
    resolve: desktopVitestResolve,
  }),
);
