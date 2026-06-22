import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import importMetaUrlPlugin from "@codingame/esbuild-import-meta-url-plugin";
import fs from "node:fs";
import path from "path";

const host = process.env.TAURI_DEV_HOST;
const apiProxyTarget = process.env.VITE_API_PROXY_TARGET;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [
    react(), tailwindcss(),
    {
      name: 'load-vscode-css-as-string',
      enforce: 'pre',
      async resolveId(source, importer, options) {
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
    {
      // For the *-language-features extensions which use SharedArrayBuffer
      name: 'configure-response-headers',
      apply: 'serve',
      configureServer: (server) => {
        server.middlewares.use((_req, res, next) => {
          res.setHeader('Cross-Origin-Embedder-Policy', 'credentialless')
          res.setHeader('Cross-Origin-Opener-Policy', 'same-origin')
          res.setHeader('Cross-Origin-Resource-Policy', 'cross-origin')
          next()
        })
      }
    },
    {
      name: 'force-prevent-transform-assets',
      apply: 'serve',
      configureServer(server) {
        return () => {
          server.middlewares.use((req, res, next) => {
            if (req.originalUrl != null) {
              const pathname = new URL(req.originalUrl, import.meta.url).pathname
              if (pathname.endsWith('.html')) {
                res.setHeader('Content-Type', 'text/html')
                res.writeHead(200)
                res.write(fs.readFileSync(path.join(__dirname, pathname)))
                res.end()
                return
              }
            }

            next()
          })
        }
      }
    }
  ],
  optimizeDeps: {
      include: [

      '@codingame/monaco-vscode-api/extensions',
      '@codingame/monaco-vscode-api',
      '@codingame/monaco-vscode-api/monaco',
      'vscode/localExtensionHost',

      // These 2 lines prevent vite from reloading the whole page when starting a worker (so 2 times in a row after cleaning the vite cache - for the editor then the textmate workers)
      // it's mainly empirical and probably not the best way, fix me if you find a better way
      // '@vscode/vscode-languagedetection',
      // 'marked'
    ],
    esbuildOptions: {
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
            return "workspace-lsp-client";
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
  // Path alias configuration
  resolve: {
    dedupe: ["@tanstack/react-query", "monaco-editor", "vscode"],
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@slab/api/config": path.resolve(__dirname, "../api/src/config.ts"),
      "@slab/api/errors": path.resolve(__dirname, "../api/src/errors.ts"),
      "@slab/api/models": path.resolve(__dirname, "../api/src/models.ts"),
      "@slab/api/permissions": path.resolve(__dirname, "../api/src/permissions.ts"),
      "@slab/api/plugin": path.resolve(__dirname, "../api/src/plugin.ts"),
      "@slab/api/v1": path.resolve(__dirname, "../api/src/v1.d.ts"),
      "@slab/api": path.resolve(__dirname, "../api/src/index.ts"),
      vscode: path.resolve(__dirname, "./node_modules/vscode"),
    },
  },
  test: {
    typecheck: {
      enabled: true,
      tsconfig: './tsconfig.json',
    },
  },
}));
