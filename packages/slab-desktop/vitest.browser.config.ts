import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { playwright } from "@vitest/browser-playwright";
import { defineProject } from "vitest/config";

import { desktopVitestResolve } from "./vitest.shared";

const browserActionTimeoutMs = 5_000;
const browserTestTimeoutMs = 30_000;
const browserTeardownTimeoutMs = 10_000;
const screenshotAllowedMismatchedPixelRatio = 0.0005;

export default defineProject({
  plugins: [react(), tailwindcss()],
  optimizeDeps: {
    include: ["react-dom/client"],
  },
  test: {
    name: "desktop-browser",
    include: ["tests/browser/**/*.browser.test.tsx"],
    css: true,
    fileParallelism: false,
    hookTimeout: browserTestTimeoutMs,
    retry: 0,
    setupFiles: ["./tests/browser/vitest.setup.ts"],
    teardownTimeout: browserTeardownTimeoutMs,
    testTimeout: browserTestTimeoutMs,
    browser: {
      enabled: true,
      headless: true,
      api: { host: "127.0.0.1", port: 64115 },
      expect: {
        toMatchScreenshot: {
          comparatorOptions: {
            allowedMismatchedPixelRatio: screenshotAllowedMismatchedPixelRatio,
            threshold: 0.1,
          },
          timeout: browserActionTimeoutMs,
        },
      },
      provider: playwright({
        actionTimeout: browserActionTimeoutMs,
      }),
      viewport: {
        width: 1440,
        height: 960,
      },
      instances: [{ browser: "chromium" }],
    },
  },
  resolve: desktopVitestResolve,
});
