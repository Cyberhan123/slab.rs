import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import JSZip from "jszip";
import { describe, expect, it } from "vitest";

import { generatePluginPacks } from "../../../scripts/plugins/generate-plugin-packs";

describe("plugin pack generation", () => {
  it("writes integrity only into the packaged manifest", async () => {
    const root = await createTempRoot("basic");
    try {
      const pluginRoot = path.join(root, "plugins", "sample-plugin");
      await writePluginFile(pluginRoot, "ui/index.html", "<!doctype html>");
      await writePluginFile(pluginRoot, "dist/plugin.js", "export function run() { return null; }");
      const sourceManifest = {
        manifestVersion: 1,
        id: "sample-plugin",
        name: "Sample Plugin",
        version: "0.1.0",
        runtime: {
          ui: { entry: "ui/index.html" },
          js: { entry: "dist/plugin.js" },
        },
      };
      await writeFile(path.join(pluginRoot, "plugin.json"), JSON.stringify(sourceManifest));
      await writePluginFile(pluginRoot, "node_modules/ignored/index.js", "ignored");
      await writePluginFile(pluginRoot, "package.json", "{\"name\":\"ignored\"}");

      const archives = await generatePluginPacks({
        outDir: path.join(root, "out"),
        pluginIds: new Set<string>(),
        pluginsDir: path.join(root, "plugins"),
      });

      const unchangedManifest = JSON.parse(
        await readFile(path.join(pluginRoot, "plugin.json"), "utf8"),
      );
      expect(unchangedManifest.integrity).toBeUndefined();
      const archive = await readArchive(archives[0]);
      const manifest = JSON.parse(
        await archive.file("sample-plugin/plugin.json")!.async("string"),
      );
      expect(Object.keys(manifest.integrity.filesSha256)).toEqual([
        "dist/plugin.js",
        "ui/index.html",
      ]);
      expect(archive.file("sample-plugin/package.json")).toBeNull();
      expect(archive.file("sample-plugin/node_modules/ignored/index.js")).toBeNull();
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  it("keeps package.json for nodePackage language server plugins", async () => {
    const root = await createTempRoot("node-package");
    try {
      const pluginRoot = path.join(root, "plugins", "lsp-plugin");
      await writePluginFile(pluginRoot, "ui/index.html", "<!doctype html>");
      await writePluginFile(pluginRoot, "package.json", "{\"name\":\"lsp-plugin\"}");
      await writeFile(
        path.join(pluginRoot, "plugin.json"),
        JSON.stringify({
          manifestVersion: 1,
          id: "lsp-plugin",
          name: "LSP Plugin",
          version: "0.1.0",
          runtime: { ui: { entry: "ui/index.html" } },
          contributes: {
            languageServers: [
              {
                id: "lsp-plugin.typescript",
                languages: ["typescript"],
                transport: { type: "nodePackage", package: "typescript-language-server" },
              },
            ],
          },
        }),
      );

      const archives = await generatePluginPacks({
        outDir: path.join(root, "out"),
        pluginIds: new Set<string>(),
        pluginsDir: path.join(root, "plugins"),
      });

      const archive = await readArchive(archives[0]);
      const manifest = JSON.parse(await archive.file("lsp-plugin/plugin.json")!.async("string"));
      expect(Object.keys(manifest.integrity.filesSha256)).toEqual([
        "package.json",
        "ui/index.html",
      ]);
      expect(archive.file("lsp-plugin/package.json")).not.toBeNull();
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});

async function createTempRoot(label: string): Promise<string> {
  return mkdtemp(path.join(os.tmpdir(), `slab-plugin-pack-${label}-`));
}

async function writePluginFile(root: string, relativePath: string, content: string): Promise<void> {
  const absolutePath = path.join(root, relativePath);
  await mkdir(path.dirname(absolutePath), { recursive: true });
  await writeFile(absolutePath, content);
}

async function readArchive(archivePath: string): Promise<JSZip> {
  return JSZip.loadAsync(await readFile(archivePath));
}
