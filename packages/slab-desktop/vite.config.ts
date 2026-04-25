import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],

  build: {
    // Tauri on macOS uses the system WebKit. Our minimum supported macOS
    // version is 13.0, so keep the frontend output within the Safari 16
    // feature set instead of following Vite's moving default baseline.
    target: "safari16",
    cssTarget: "safari16",
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
  // Path alias configuration
  resolve: {
    dedupe: ["@tanstack/react-query"],
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@slab/api/config": path.resolve(__dirname, "../api/src/config.ts"),
      "@slab/api/errors": path.resolve(__dirname, "../api/src/errors.ts"),
      "@slab/api/models": path.resolve(__dirname, "../api/src/models.ts"),
      "@slab/api/permissions": path.resolve(__dirname, "../api/src/permissions.ts"),
      "@slab/api/plugin": path.resolve(__dirname, "../api/src/plugin.ts"),
      "@slab/api/v1": path.resolve(__dirname, "../api/src/v1.d.ts"),
      "@slab/api": path.resolve(__dirname, "../api/src/index.ts"),
    },
  },
  test: {
    typecheck: {
      enabled: true,
      tsconfig: './tsconfig.json',
    },
  },
}));
