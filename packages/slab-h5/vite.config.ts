import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const apiProxyTarget = process.env.VITE_API_PROXY_TARGET;

// H5 shell: same source-alias strategy as the desktop shell so workspace
// packages never need a build step during dev.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    dedupe: ["@tanstack/react-query"],
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
        find: /^@\/(.+)$/,
        replacement: path.resolve(__dirname, "../slab-components/src").replace(/\\/g, "/") + "/$1",
      },
    ],
  },
  server: {
    port: 1440,
    strictPort: true,
    proxy: apiProxyTarget
      ? {
          "/v1": {
            target: apiProxyTarget,
            changeOrigin: true,
            ws: true,
          },
        }
      : undefined,
  },
});
