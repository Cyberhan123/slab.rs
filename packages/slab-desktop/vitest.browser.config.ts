import path from "node:path";

import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { playwright } from "@vitest/browser-playwright";
import { defineProject } from "vitest/config";

const componentSourcePath = path.resolve(__dirname, "../slab-components/src");
const componentSourceUrl = componentSourcePath.replace(/\\/g, "/");

export default defineProject({
  plugins: [react(), tailwindcss()],
  test: {
    name: "desktop-browser",
    include: ["tests/browser/**/*.browser.test.tsx"],
    css: true,
    fileParallelism: false,
    setupFiles: ["./tests/browser/vitest.setup.ts"],
    browser: {
      enabled: true,
      headless: true,
      provider: playwright({
        actionTimeout: 5_000,
      }),
      viewport: {
        width: 1440,
        height: 960,
      },
      instances: [{ browser: "chromium" }],
    },
  },
  resolve: {
    dedupe: ["react", "react-dom"],
    alias: [
      {
        find: "@slab/components/globals.css",
        replacement: path.resolve(componentSourcePath, "styles/globals.css"),
      },
      {
        find: /^@slab\/components\/(.+)$/,
        replacement: `${componentSourceUrl}/$1`,
      },
      {
        find: "@slab/components",
        replacement: path.resolve(componentSourcePath, "index.ts"),
      },
      {
        find: "@slab/i18n",
        replacement: path.resolve(__dirname, "../slab-i18n/src/index.ts"),
      },
      {
        find: "@",
        replacement: path.resolve(__dirname, "./src"),
      },
    ],
  },
});
