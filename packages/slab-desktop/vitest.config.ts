import path from "path";
import react from "@vitejs/plugin-react";
import { defineProject } from "vitest/config";

const componentSourcePath = path.resolve(__dirname, "../slab-components/src");
const componentSourceUrl = componentSourcePath.replace(/\\/g, "/");
const apiSourcePath = path.resolve(__dirname, "../api/src");
const apiSourceUrl = apiSourcePath.replace(/\\/g, "/");

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
    ],
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
        find: /^@slab\/api\/(.+)$/,
        replacement: `${apiSourceUrl}/$1`,
      },
      {
        find: "@slab/api",
        replacement: path.resolve(apiSourcePath, "index.ts"),
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
