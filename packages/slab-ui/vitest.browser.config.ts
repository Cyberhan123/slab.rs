import react from "@vitejs/plugin-react";
import { playwright } from "@vitest/browser-playwright";
import { defineProject, mergeConfig } from "vitest/config";

import { uiVitestResolve } from "./vitest.shared";
import { vitestBase } from "../../vitest.base";

// Dedicated screenshot/visual-regression project (mirrors
// packages/slab-desktop/vitest.browser.config.ts). Port 64117 is NOT pinned
// here because the sibling `ui` project already claims it; Playwright picks a
// free port.
const browserActionTimeoutMs = 5_000;
const browserTestTimeoutMs = 30_000;

export default defineProject(
  mergeConfig(vitestBase, {
    plugins: [react()],
    test: {
      name: "ui-browser",
      css: true,
      include: ["src/**/*.browser.test.*"],
      hookTimeout: browserTestTimeoutMs,
      testTimeout: browserTestTimeoutMs,
      browser: {
        enabled: true,
        headless: true,
        provider: playwright({
          actionTimeout: browserActionTimeoutMs,
        }),
        instances: [{ browser: "chromium" }],
      },
    },
    resolve: uiVitestResolve,
  }),
);
