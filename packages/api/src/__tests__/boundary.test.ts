import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import * as apiNamespace from "../index";

const fromHere = (relative: string) =>
  fileURLToPath(new URL(relative, import.meta.url));

describe("@slab/api boundary", () => {
  it("does not ship the plugin API surface sources", () => {
    // Plugin-specific code moved to @slab/plugin-sdk. These files must not be
    // re-created here.
    expect(existsSync(fromHere("../plugin.ts"))).toBe(false);
    expect(existsSync(fromHere("../permissions.ts"))).toBe(false);
  });

  it("does not expose plugin subpath exports", () => {
    const packageJson = JSON.parse(
      readFileSync(fromHere("../../package.json"), "utf8"),
    ) as { exports: Record<string, unknown> };
    expect(packageJson.exports).not.toHaveProperty("./plugin");
    expect(packageJson.exports).not.toHaveProperty("./permissions");
  });

  it("does not re-export plugin API symbols from the barrel", () => {
    const surface = apiNamespace as Record<string, unknown>;
    expect(surface.createSlabPluginApiFetch).toBeUndefined();
    expect(surface.createSlabPluginApiClient).toBeUndefined();
    expect(surface.requiredSlabApiPermission).toBeUndefined();
    expect(surface.SLAB_API_PERMISSIONS).toBeUndefined();
    expect(surface.assertSlabPluginApiSurface).toBeUndefined();
    expect(surface.describeSlabApiPermission).toBeUndefined();
  });
});
