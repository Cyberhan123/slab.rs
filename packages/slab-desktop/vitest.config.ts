import path from "path";
import react from "@vitejs/plugin-react";
import { defineProject } from "vitest/config";

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
      "tests/e2e/**",
    ],
  },
  resolve: {
    dedupe: ["react", "react-dom"],
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@slab/components": path.resolve(__dirname, "../slab-components/src/index.ts"),
      "@slab/components/*": path.resolve(__dirname, "../slab-components/src/*"),
      "@slab/i18n": path.resolve(__dirname, "../slab-i18n/src/index.ts"),
    },
  },
});
