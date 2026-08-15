import react from "@vitejs/plugin-react";
import { playwright } from "@vitest/browser-playwright";
import { defineProject, mergeConfig } from "vitest/config";

import { vitestBase } from "../../vitest.base";

// Browser-mode unit tests (mirrors packages/slab-desktop/vitest.config.ts):
// `render`/`renderHook` come from `vitest-browser-react`, which only works in
// Browser Mode, so this project drives a real headless Chromium via Playwright.
const browserActionTimeoutMs = 5_000;
const browserTestTimeoutMs = 30_000;

export default defineProject(
  mergeConfig(vitestBase, {
    plugins: [react()],
    test: {
      name: "ui",
      css: true,
      hookTimeout: browserTestTimeoutMs,
      testTimeout: browserTestTimeoutMs,
      exclude: ["src/**/*.browser.test.*"],
      browser: {
        enabled: true,
        headless: true,
        api: { port: 64117 },
        provider: playwright({
          actionTimeout: browserActionTimeoutMs,
        }),
        instances: [{ browser: "chromium" }],
      },
    },
  }),
);
