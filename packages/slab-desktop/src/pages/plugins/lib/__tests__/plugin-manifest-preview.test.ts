import JSZip from "jszip";
import { describe, expect, it } from "vitest";

import { parsePluginPackManifest } from "../plugin-manifest-preview";

async function makePack(files: Record<string, string>): Promise<File> {
  const zip = new JSZip();
  for (const [path, content] of Object.entries(files)) {
    zip.file(path, content);
  }
  const blob = await zip.generateAsync({ type: "blob" });
  return new File([blob], "test.plugin.slab", { type: "application/zip" });
}

describe("parsePluginPackManifest", () => {
  it("extracts declared permissions from a root-level plugin.json", async () => {
    const file = await makePack({
      "demo-plugin/plugin.json": JSON.stringify({
        manifestVersion: 1,
        id: "demo-plugin",
        name: "Demo",
        version: "0.1.0",
        permissions: {
          network: { mode: "allowlist", allowHosts: ["example.com"] },
          slabApi: ["chat:complete", "models:read"],
          files: { read: ["video"], write: ["workspace"] },
          agent: ["hook:declare"],
          lsp: ["languageServer:declare"],
        },
      }),
      "demo-plugin/ui/index.html": "<!doctype html>",
    });

    const preview = await parsePluginPackManifest(file);

    expect(preview).not.toBeNull();
    expect(preview?.id).toBe("demo-plugin");
    expect(preview?.name).toBe("Demo");
    expect(preview?.version).toBe("0.1.0");
    expect(preview?.permissions.slabApi).toEqual(["chat:complete", "models:read"]);
    expect(preview?.permissions.filesRead).toEqual(["video"]);
    expect(preview?.permissions.filesWrite).toEqual(["workspace"]);
    expect(preview?.permissions.networkMode).toBe("allowlist");
    expect(preview?.permissions.networkHosts).toEqual(["example.com"]);
    expect(preview?.permissions.agent).toEqual(["hook:declare"]);
    expect(preview?.permissions.lsp).toEqual(["languageServer:declare"]);
    expect(preview?.parseError).toBeNull();
  });

  it("prefers the shallowest manifest when the pack nests the plugin dir", async () => {
    const file = await makePack({
      "archive-root/demo-plugin/plugin.json": JSON.stringify({
        id: "demo-plugin",
        name: "Nested",
        version: "1.0.0",
        permissions: { slabApi: ["tasks:read"] },
      }),
    });

    const preview = await parsePluginPackManifest(file);

    expect(preview?.name).toBe("Nested");
    expect(preview?.permissions.slabApi).toEqual(["tasks:read"]);
  });

  it("returns null for a non-zip file or a pack without a manifest", async () => {
    const blob = new Blob(["not a zip"], { type: "application/octet-stream" });
    expect(await parsePluginPackManifest(new File([blob], "bad.plugin.slab"))).toBeNull();

    const empty = await makePack({ "readme.txt": "no manifest here" });
    expect(await parsePluginPackManifest(empty)).toBeNull();
  });

  it("reports a parse error when plugin.json is not valid JSON", async () => {
    const file = await makePack({ "broken/plugin.json": "{ not json" });

    const preview = await parsePluginPackManifest(file);

    expect(preview?.parseError).not.toBeNull();
    expect(preview?.permissions.slabApi).toEqual([]);
  });
});
