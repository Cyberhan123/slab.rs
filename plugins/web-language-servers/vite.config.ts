import { readdirSync, readFileSync, statSync } from "node:fs";
import { builtinModules, createRequire } from "node:module";
import path from "node:path";
import { defineConfig } from "vite";

const require = createRequire(import.meta.url);
const typescriptLanguageServerPackagePath = require.resolve("typescript-language-server/package.json");
const typescriptLanguageServerPackage = JSON.parse(
  readFileSync(typescriptLanguageServerPackagePath, "utf8"),
) as { name?: string; type?: string; version?: string };
const typescriptPackagePath = require.resolve("typescript/package.json");
const typescriptLibPath = path.join(path.dirname(require.resolve("typescript")), ".");

function withSlabImmediateShim(source: string) {
  if (!source.includes("setImmediate") && !source.includes("clearImmediate")) {
    return source;
  }

  return `const setImmediate = (callback, ...args) => setTimeout(callback, 0, ...args);
const clearImmediate = (handle) => clearTimeout(handle);
${source}`;
}

function withSlabTypeScriptResolution(source: string) {
  return source.replace(
    'const file = require.resolve("typescript");',
    'const file = path__default.join(path__default.dirname(require.resolve("typescript/package.json")), "lib", "typescript.js");',
  );
}

function withSlabTypeScriptServerStdio(source: string) {
  return source.replace(
    "const useIpc = version.version?.gte(API.v490);",
    "const useIpc = false;",
  );
}

function withSlabTypeScriptSystemImmediate(source: string) {
  return source.replaceAll(
    "      setTimeout,\n      clearTimeout,",
    "      setTimeout,\n      clearTimeout,\n      setImmediate,\n      clearImmediate,",
  );
}

function withSlabTypeScriptServerHostFallback(source: string) {
  return source.replace(
    'this.host.setImmediate(() => this.event({ pid: this.installer.pid }, "typingsInstallerPid"));',
    '(this.host.setImmediate ?? this.host.setTimeout)(() => this.event({ pid: this.installer.pid }, "typingsInstallerPid"), 0);',
  );
}

function withSlabLanguageServerRuntimeShims(source: string) {
  return withSlabImmediateShim(
    withSlabTypeScriptServerHostFallback(
      withSlabTypeScriptSystemImmediate(
        withSlabTypeScriptServerStdio(withSlabTypeScriptResolution(source)),
      ),
    ),
  );
}

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
      plugins: [
        {
          name: "slab-typescript-language-server-package-metadata",
          renderChunk(code) {
            return withSlabLanguageServerRuntimeShims(code);
          },
          generateBundle() {
            this.emitFile({
              type: "asset",
              fileName: "package.json",
              source: `${JSON.stringify({
                name: typescriptLanguageServerPackage.name ?? "typescript-language-server",
                type: typescriptLanguageServerPackage.type ?? "module",
                version: typescriptLanguageServerPackage.version,
              }, null, 2)}\n`,
            });
            this.emitFile({
              type: "asset",
              fileName: "node_modules/typescript/package.json",
              source: readFileSync(typescriptPackagePath),
            });

            const pending = [typescriptLibPath];
            while (pending.length > 0) {
              const current = pending.pop();
              if (!current) {
                continue;
              }

              for (const entry of readdirSync(current)) {
                const sourcePath = path.join(current, entry);
                const stats = statSync(sourcePath);
                if (stats.isDirectory()) {
                  pending.push(sourcePath);
                  continue;
                }

                const source = readFileSync(sourcePath);
                this.emitFile({
                  type: "asset",
                  fileName: path.posix.join(
                    "node_modules/typescript/lib",
                    path.relative(typescriptLibPath, sourcePath).split(path.sep).join(path.posix.sep),
                  ),
                  source: sourcePath.endsWith(".js")
                    ? withSlabLanguageServerRuntimeShims(source.toString("utf8"))
                    : source,
                });
              }
            }
          },
        },
      ],
    },
    sourcemap: true,
    target: "es2022",
  },
  ssr: {
    noExternal: true,
  },
});
