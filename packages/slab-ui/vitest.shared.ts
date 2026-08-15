import path from "node:path";

const componentSourcePath = path.resolve(__dirname, "../slab-components/src");
const componentSourceUrl = componentSourcePath.replace(/\\/g, "/");
const apiSourcePath = path.resolve(__dirname, "../api/src");
const apiSourceUrl = apiSourcePath.replace(/\\/g, "/");
const coreSourcePath = path.resolve(__dirname, "../slab-core/src");
const coreSourceUrl = coreSourcePath.replace(/\\/g, "/");
const testUtilsSourcePath = path.resolve(__dirname, "../slab-test-utils/src");
const testUtilsSourceUrl = testUtilsSourcePath.replace(/\\/g, "/");

export const uiVitestResolve = {
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
      find: /^@slab\/test-utils\/(.+)$/,
      replacement: `${testUtilsSourceUrl}/$1`,
    },
    {
      find: "@slab/test-utils",
      replacement: path.resolve(testUtilsSourcePath, "index.ts"),
    },
    {
      find: "@slab/plugin-sdk",
      replacement: path.resolve(__dirname, "../slab-plugin-sdk/src/index.ts"),
    },
    {
      find: "@slab/i18n",
      replacement: path.resolve(__dirname, "../slab-i18n/src/index.ts"),
    },
    {
      find: /^@slab\/ui\/(.+)$/,
      replacement: path.resolve(__dirname, "./src").replace(/\\/g, "/") + "/$1",
    },
    {
      find: "@slab/ui",
      replacement: path.resolve(__dirname, "./src/index.ts"),
    },
    {
      // `@` belongs to @slab/components sources (they import `@/lib/utils`).
      find: /^@\/(.+)$/,
      replacement: `${componentSourceUrl}/$1`,
    },
  ],
};
