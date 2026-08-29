import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";
import importMetaUrlPlugin from "./src/lib/vite-plugin-dev-url-meta-for-vscode"

const host = process.env.TAURI_DEV_HOST;
const apiProxyTarget = process.env.VITE_API_PROXY_TARGET;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [
    react(), 
    tailwindcss(),
    {
      name: 'load-vscode-css-as-string',
      enforce: 'pre',
      async resolveId(this, source, importer, options) {
        const resolved = (await this.resolve(source, importer, options))!
        if (
          resolved.id.match(
            /node_modules\/(@codingame\/monaco-vscode|vscode|monaco-editor).*\.css$/
          )
        ) {
          return {
            ...resolved,
            id: resolved.id + '?inline'
          }
        }
        return undefined
      }
    },
  ],
  optimizeDeps: {
    include: [
      'vscode-textmate',
      'vscode-oniguruma',
      '@vscode/vscode-languagedetection',
      '@codingame/monaco-vscode-api/extensions',
      '@codingame/monaco-vscode-api',
      '@codingame/monaco-vscode-api/monaco',
      'vscode/localExtensionHost',

      // These 2 lines prevent vite from reloading the whole page when starting a worker (so 2 times in a row after cleaning the vite cache - for the editor then the textmate workers)
      // it's mainly empirical and probably not the best way, fix me if you find a better way
      // '@vscode/vscode-languagedetection',
      // 'marked'
    ],
    rolldownOptions: {
      tsconfig: './tsconfig.json',
      plugins: [importMetaUrlPlugin],
    },
  },

  build: {
    // Tauri on macOS uses the system WebKit. Our minimum supported macOS
    // version is 13.0, so keep the frontend output within the Safari 16
    // feature set instead of following Vite's moving default baseline.
    target: "safari16",
    cssTarget: "safari16",
    rolldownOptions: {
      output: {
        manualChunks(id) {
          if (
            id.includes("vscode-languageclient") ||
            id.includes("vscode-ws-jsonrpc") ||
            id.includes("@codingame/monaco-editor") ||
            id.includes("@codingame/monaco-vscode") ||
            id.includes("/node_modules/vscode/")
          ) {
            return "vscode-services";
          }
          return undefined;
        },
      },
    },
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
    proxy: apiProxyTarget
      ? {
        "/v1": {
          target: apiProxyTarget,
          changeOrigin: true,
          ws: true,
        },
      }
      : undefined,
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
  // Path alias configuration. Array form (exact entries before their package
  // root) so extensionless deep imports like `@slab/core/harness` resolve to
  // workspace sources without a package build step. Mirrors vitest.shared.ts.
  resolve: {
    // `@codingame/monaco-vscode-api` must resolve to ONE physical copy: its
    // services.js throws at import time when a second copy (stale nested
    // node_modules link) enters the bundle, which white-screens the app before
    // React mounts.
    dedupe: [
      "@tanstack/react-query",
      "monaco-editor",
      "vscode",
      "@codingame/monaco-vscode-api",
    ],
    alias: [
      {
        find: "@slab/components/globals.css",
        replacement: path.resolve(__dirname, "../slab-components/src/styles/globals.css"),
      },
      {
        find: /^@slab\/components\/(.+)$/,
        replacement: path.resolve(__dirname, "../slab-components/src").replace(/\\/g, "/") + "/$1",
      },
      {
        find: "@slab/components",
        replacement: path.resolve(__dirname, "../slab-components/src/index.ts"),
      },
      {
        find: /^@slab\/api\/(.+)$/,
        replacement: path.resolve(__dirname, "../api/src").replace(/\\/g, "/") + "/$1",
      },
      {
        find: "@slab/api",
        replacement: path.resolve(__dirname, "../api/src/index.ts"),
      },
      {
        find: /^@slab\/core\/(.+)$/,
        replacement: path.resolve(__dirname, "../slab-core/src").replace(/\\/g, "/") + "/$1",
      },
      {
        find: "@slab/core",
        replacement: path.resolve(__dirname, "../slab-core/src/index.ts"),
      },
      {
        find: /^@slab\/ui\/(.+)$/,
        replacement: path.resolve(__dirname, "../slab-ui/src").replace(/\\/g, "/") + "/$1",
      },
      {
        find: "@slab/ui",
        replacement: path.resolve(__dirname, "../slab-ui/src/index.ts"),
      },
      {
        // `@` belongs to @slab/components sources (they import `@/lib/utils`);
        // desktop's own files use relative or @slab/* specifiers.
        find: /^@\/(.+)$/,
        replacement: path.resolve(__dirname, "../slab-components/src").replace(/\\/g, "/") + "/$1",
      },
      {
        find: "vscode",
        replacement: path.resolve(__dirname, "./node_modules/vscode"),
      },
    ],
  },
  test: {
    typecheck: {
      enabled: true,
      tsconfig: './tsconfig.json',
    },
  },
}));
