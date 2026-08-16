import react from "@vitejs/plugin-react";
import { playwright } from "@vitest/browser-playwright";
import { defineProject, mergeConfig } from "vitest/config";

import { desktopVitestResolve } from "./vitest.shared";
import { vitestBase } from "../../vitest.base";

// Browser-mode unit tests. `render`/`renderHook` come from `vitest-browser-react`
// (which only works in Browser Mode), so this project drives a real headless
// Chromium via Playwright rather than jsdom. No `api.port` is pinned: the
// sibling `desktop-browser` (64115) and `components-browser` (64116) projects
// already claim those, and vitest/Playwright can pick a free port here.
const browserActionTimeoutMs = 5_000;
const browserTestTimeoutMs = 30_000;

export default defineProject(
  mergeConfig(vitestBase, {
    plugins: [react()],
    test: {
      name: "desktop",
      css: true,
      hookTimeout: browserTestTimeoutMs,
      testTimeout: browserTestTimeoutMs,
      // vitestBase already excludes node_modules / dist / config files; add the
      // desktop-only trees that are exercised by the browser/e2e projects.
      exclude: ["tests/browser/**", "tests/e2e/**", "tests/manual/**"],
      browser: {
        enabled: true,
        headless: true,
        provider: playwright({
          actionTimeout: browserActionTimeoutMs,
        }),
        instances: [{ browser: "chromium" }],
      },
    },
    resolve: desktopVitestResolve,
  }),
);
