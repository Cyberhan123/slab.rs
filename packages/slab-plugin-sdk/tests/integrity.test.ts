import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";

import { computePluginIntegrity, updatePluginManifestIntegrity } from "../src/integrity";

describe("plugin integrity", () => {
  it("computes stable SHA-256 entries for ui, schemas, and optional wasm runtime", async () => {
    const root = await createTempPlugin("compute");
    try {
      await writePluginFile(root, "ui/index.html", "<!doctype html>");
      await writePluginFile(root, "ui/assets/app.js", "console.log('plugin');");
      await writePluginFile(root, "schemas/input.schema.json", "{\"type\":\"object\"}");
      await writePluginFile(root, "wasm/plugin.wasm", "wasm");
      await writePluginFile(root, "src/ignored.ts", "ignored");

      const hashes = await computePluginIntegrity(root);

      expect(Object.keys(hashes)).toEqual([
        "schemas/input.schema.json",
        "ui/assets/app.js",
        "ui/index.html",
        "wasm/plugin.wasm",
      ]);
      expect(hashes["src/ignored.ts"]).toBeUndefined();
      expect(hashes["ui/index.html"]).toMatch(/^[a-f0-9]{64}$/);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  it("updates plugin.json with a sorted filesSha256 map", async () => {
    const root = await createTempPlugin("manifest");
    try {
      await writePluginFile(root, "ui/index.html", "<!doctype html>");
      await writePluginFile(root, "schemas/settings.schema.json", "{\"type\":\"object\"}");
      await writeFile(
        path.join(root, "plugin.json"),
        JSON.stringify({
          manifestVersion: 1,
          id: "sample",
          name: "Sample",
          version: "0.1.0",
          integrity: { filesSha256: { "ui/old.js": "old" } },
        }),
      );

      await updatePluginManifestIntegrity(root);
      const manifest = JSON.parse(await readFile(path.join(root, "plugin.json"), "utf8"));

      expect(Object.keys(manifest.integrity.filesSha256)).toEqual([
        "schemas/settings.schema.json",
        "ui/index.html",
      ]);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});

async function createTempPlugin(label: string): Promise<string> {
  return mkdtemp(path.join(os.tmpdir(), `slab-plugin-sdk-${label}-`));
}

async function writePluginFile(root: string, relativePath: string, content: string): Promise<void> {
  const absolutePath = path.join(root, relativePath);
  await mkdir(path.dirname(absolutePath), { recursive: true });
  await writeFile(absolutePath, content);
}
