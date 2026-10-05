import path from "node:path";

const componentSourcePath = path.resolve(import.meta.dirname, "../slab-components/src");
const componentSourceUrl = componentSourcePath.replace(/\\/g, "/");
const apiSourcePath = path.resolve(import.meta.dirname, "../api/src");
const apiSourceUrl = apiSourcePath.replace(/\\/g, "/");
const testUtilsSourcePath = path.resolve(import.meta.dirname, "../slab-test-utils/src");
const testUtilsSourceUrl = testUtilsSourcePath.replace(/\\/g, "/");
const coreSourcePath = path.resolve(import.meta.dirname, "../slab-core/src");
const coreSourceUrl = coreSourcePath.replace(/\\/g, "/");
const uiSourcePath = path.resolve(import.meta.dirname, "../slab-ui/src");
const uiSourceUrl = uiSourcePath.replace(/\\/g, "/");

export const desktopVitestResolve = {
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
      find: /^@slab\/core\/(.+)$/,
      replacement: `${coreSourceUrl}/$1`,
    },
    {
      find: "@slab/core",
      replacement: path.resolve(coreSourcePath, "index.ts"),
    },
    {
      find: /^@slab\/ui\/(.+)$/,
      replacement: `${uiSourceUrl}/$1`,
    },
    {
      find: "@slab/ui",
      replacement: path.resolve(uiSourcePath, "index.ts"),
    },
    {
      find: "@slab/plugin-sdk",
      replacement: path.resolve(import.meta.dirname, "../slab-plugin-sdk/src/index.ts"),
    },
    {
      find: "@slab/i18n",
      replacement: path.resolve(import.meta.dirname, "../slab-i18n/src/index.ts"),
    },
    {
      find: /^@slab\/test-utils\/(.+)$/,
      replacement: `${testUtilsSourceUrl}/$1`,
    },
    {
      find: "@slab/test-utils",
      replacement: path.resolve(testUtilsSourcePath, "index.ts"),
    },
    {
      // `@` belongs to @slab/components sources (they import `@/lib/utils`);
      // desktop's own files use relative or @slab/* specifiers.
      find: /^@\/(.+)$/,
      replacement: path.resolve(import.meta.dirname, "../slab-components/src").replace(/\\/g, "/") + "/$1",
    },
  ],
};
