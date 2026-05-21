import { beforeAll, describe, expect, it } from "vitest";

import type { SlabServerTestHarness } from "../../support/slab-server";
import { expectError, expectJson, jsonInit, type Schema } from "./shared";

export function registerPluginsSmoke(getServer: () => SlabServerTestHarness): void {
  describe("slab-server smoke plugins", () => {
    let server: SlabServerTestHarness;

    beforeAll(() => {
      server = getServer();
    });

    it("covers plugin management routes with validation and not-found paths", async () => {
      const plugins = await expectJson<Schema["PluginResponse"][]>(server, "/v1/plugins");
      expect(plugins.response.ok).toBe(true);
      expect(Array.isArray(plugins.body)).toBe(true);

      await expectError(server, "/v1/plugins/missing-plugin", 404);
      await expectError(
        server,
        "/v1/plugins/install",
        400,
        jsonInit(
          {
            pluginId: ""
          } satisfies Schema["InstallPluginRequest"],
          { method: "POST" }
        )
      );

      const emptyPack = new FormData();
      const importPack = await server.requestFormData("/v1/plugins/import-pack", emptyPack, {
        method: "POST"
      });
      expect(importPack.status).toBe(400);

      for (const path of [
        "/v1/plugins/missing-plugin/enable",
        "/v1/plugins/missing-plugin/disable",
        "/v1/plugins/missing-plugin/start"
      ]) {
        await expectError(server, path, 404, { method: "POST" });
      }

      await expectError(
        server,
        "/v1/plugins/missing-plugin/stop",
        404,
        jsonInit({ lastError: "smoke" } satisfies Schema["StopPluginRequest"], { method: "POST" })
      );
      await expectError(server, "/v1/plugins/missing-plugin", 404, { method: "DELETE" });
    });
  });
}
