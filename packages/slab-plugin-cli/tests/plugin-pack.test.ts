import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import JSZip from "jszip";
import { describe, expect, it } from "vitest";

import { packPlugin, parsePackArgs } from "../src";

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

      const archivePath = await packPlugin({
        outDir: path.join(root, "out"),
        pluginDir: pluginRoot,
      });

      const unchangedManifest = JSON.parse(
        await readFile(path.join(pluginRoot, "plugin.json"), "utf8"),
      );
      expect(unchangedManifest.integrity).toBeUndefined();
      const archive = await readArchive(archivePath);
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

      const archivePath = await packPlugin({
        outDir: path.join(root, "out"),
        pluginDir: pluginRoot,
      });

      const archive = await readArchive(archivePath);
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

  it("builds a source JS backend entry into dist/plugin.js", async () => {
    const root = await createTempRoot("js-source");
    try {
      const pluginRoot = path.join(root, "plugins", "js-plugin");
      await writePluginFile(pluginRoot, "ui/index.html", "<!doctype html>");
      await writePluginFile(
        pluginRoot,
        "src/plugin.ts",
        "export function run(params: unknown) { return params; }",
      );
      await writeFile(
        path.join(pluginRoot, "plugin.json"),
        JSON.stringify({
          manifestVersion: 1,
          id: "js-plugin",
          name: "JS Plugin",
          version: "0.1.0",
          runtime: {
            ui: { entry: "ui/index.html" },
            js: { entry: "src/plugin.ts" },
          },
        }),
      );

      const archivePath = await packPlugin({
        outDir: path.join(root, "out"),
        pluginDir: pluginRoot,
      });

      const archive = await readArchive(archivePath);
      const manifest = JSON.parse(await archive.file("js-plugin/plugin.json")!.async("string"));
      expect(manifest.runtime.js.entry).toBe("dist/plugin.js");
      expect(archive.file("js-plugin/dist/plugin.js")).not.toBeNull();
      expect(Object.keys(manifest.integrity.filesSha256)).toEqual([
        "dist/plugin.js",
        "ui/index.html",
      ]);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  it("generates a Python bundle with source modules and requirements", async () => {
    const root = await createTempRoot("python");
    try {
      const pluginRoot = path.join(root, "plugins", "python-plugin");
      const dependencyRoot = path.join(root, "deps", "sample-dep");
      await writePluginFile(pluginRoot, "ui/index.html", "<!doctype html>");
      await writePluginFile(
        pluginRoot,
        "python/plugin.py",
        "from helper import inc\nfrom sample_dep import tag\n\ndef run(params):\n    return {'value': inc(params['value']), 'tag': tag()}\n",
      );
      await writePluginFile(pluginRoot, "python/helper.py", "def inc(value):\n    return value + 1\n");
      await writePluginFile(
        pluginRoot,
        "python/requirements.txt",
        `sample-dep @ file:///${dependencyRoot.replace(/\\/g, "/")}\n`,
      );
      await writePluginFile(
        dependencyRoot,
        "setup.py",
        "from setuptools import setup\nsetup(name='sample-dep', version='0.1.0', packages=['sample_dep'])\n",
      );
      await writePluginFile(
        dependencyRoot,
        "sample_dep/__init__.py",
        "def tag():\n    return 'dependency'\n",
      );
      await writeFile(
        path.join(pluginRoot, "plugin.json"),
        JSON.stringify({
          manifestVersion: 1,
          id: "python-plugin",
          name: "Python Plugin",
          version: "0.1.0",
          runtime: {
            ui: { entry: "ui/index.html" },
            python: { entry: "python/plugin.py" },
          },
        }),
      );

      const archivePath = await packPlugin({
        outDir: path.join(root, "out"),
        pluginDir: pluginRoot,
      });

      const archive = await readArchive(archivePath);
      const manifest = JSON.parse(await archive.file("python-plugin/plugin.json")!.async("string"));
      const bundle = JSON.parse(
        await archive.file("python-plugin/python/backend.slabpy")!.async("string"),
      );
      const moduleNames = bundle.modules.map((module: { name: string }) => module.name);
      expect(manifest.runtime.python.bundle).toBe("python/backend.slabpy");
      expect(bundle.entryModule).toBe("plugin");
      expect(moduleNames).toContain("helper");
      expect(moduleNames).toContain("sample_dep");
      expect(Object.keys(manifest.integrity.filesSha256)).toEqual([
        "python/backend.slabpy",
        "ui/index.html",
      ]);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  it("rejects native extensions in Python bundles", async () => {
    const root = await createTempRoot("python-native");
    try {
      const pluginRoot = path.join(root, "plugins", "python-plugin");
      await writePluginFile(pluginRoot, "ui/index.html", "<!doctype html>");
      await writePluginFile(pluginRoot, "python/plugin.py", "def run(params):\n    return params\n");
      await writePluginFile(pluginRoot, "python/native.pyd", "native");
      await writeFile(
        path.join(pluginRoot, "plugin.json"),
        JSON.stringify({
          manifestVersion: 1,
          id: "python-plugin",
          name: "Python Plugin",
          version: "0.1.0",
          runtime: {
            ui: { entry: "ui/index.html" },
            python: { entry: "python/plugin.py" },
          },
        }),
      );

      await expect(
        packPlugin({
          outDir: path.join(root, "out"),
          pluginDir: pluginRoot,
        }),
      ).rejects.toThrow("native extensions");
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});

describe("CLI argument parsing", () => {
  it("parses the pack command", () => {
    const options = parsePackArgs(
      [
        "pack",
        "--plugin-dir",
        "plugins/sample",
        "--out-dir",
        "dist",
        "--python-requirements",
        "requirements.txt",
      ],
      "C:/repo",
    );

    expect(options.pluginDir).toBe(path.resolve("C:/repo", "plugins/sample"));
    expect(options.outDir).toBe(path.resolve("C:/repo", "dist"));
    expect(options.pythonRequirements).toBe(path.resolve("C:/repo", "requirements.txt"));
  });

  it("requires the pack command and required paths", () => {
    expect(() => parsePackArgs([])).toThrow("Usage");
    expect(() => parsePackArgs(["pack", "--out-dir", "dist"])).toThrow("--plugin-dir");
    expect(() => parsePackArgs(["pack", "--plugin-dir", "plugin"])).toThrow("--out-dir");
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
