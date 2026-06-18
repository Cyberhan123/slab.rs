import react from "@vitejs/plugin-react";
import { defineProject } from "vitest/config";

import { desktopVitestResolve } from "./vitest.shared";

export default defineProject({
  plugins: [react()],
  test: {
    name: "desktop",
    globals: true,
    environment: "jsdom",
    setupFiles: ["./vitest.setup.ts"],
    css: true,
    exclude: [
      "**/node_modules/**",
      "**/dist/**",
      "tests/browser/**",
      "tests/e2e/**",
      "tests/manual/**",
    ],
  },
  resolve: desktopVitestResolve,
});
