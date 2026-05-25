import { builtinModules } from "node:module";
import path from "node:path";
import { defineConfig } from "vite";

const nodeBuiltins = new Set([
  ...builtinModules,
  ...builtinModules.map((moduleName) => `node:${moduleName}`),
]);

export default defineConfig({
  build: {
    emptyOutDir: true,
    ssr: true,
    lib: {
      entry: {
        css: path.resolve(import.meta.dirname, "src/css.ts"),
        html: path.resolve(import.meta.dirname, "src/html.ts"),
        json: path.resolve(import.meta.dirname, "src/json.ts"),
        typescript: path.resolve(import.meta.dirname, "src/typescript.ts"),
      },
      formats: ["es"],
    },
    minify: false,
    outDir: "../../bin/slab-app/src-tauri/resources/libs/language-servers/web",
    rollupOptions: {
      external: (id) => nodeBuiltins.has(id),
      output: {
        chunkFileNames: "chunks/[name]-[hash].mjs",
        entryFileNames: "[name].mjs",
      },
    },
    sourcemap: true,
    target: "es2022",
  },
  ssr: {
    noExternal: true,
  },
});
