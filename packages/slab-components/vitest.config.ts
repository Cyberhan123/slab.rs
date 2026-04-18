import path from "node:path"

import react from "@vitejs/plugin-react"
import tailwindcss from "@tailwindcss/vite"
import { playwright } from "@vitest/browser-playwright"
import { defineProject } from "vitest/config"

export default defineProject({
  plugins: [react(), tailwindcss()],
  test: {
    name: "components-browser",
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
        width: 1280,
        height: 900,
      },
      instances: [{ browser: "chromium" }],
    },
  },
  resolve: {
    dedupe: ["react", "react-dom"],
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
})
