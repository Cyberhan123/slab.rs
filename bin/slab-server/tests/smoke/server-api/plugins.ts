import { beforeAll, describe, expect, it } from "vitest";

import type { SlabServerTestHarness } from "../../support/slab-server";
import {
  buildPluginPackFile,
  expectError,
  expectJson,
  expectWebSocketJsonReply,
  expectWebSocketOpens,
  externalBaseUrl,
  formDataWithFile,
  jsonInit,
  type Schema
} from "./shared";

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

      await expectWebSocketOpens(server, "/v1/plugins/events");
      const rpcError = await expectWebSocketJsonReply<{
        error?: { code?: number; message?: string };
        id?: number | string | null;
      }>(server, "/v1/plugins/rpc", {
        id: 1,
        jsonrpc: "2.0",
        method: "missing-plugin.run",
        params: {}
      });
      expect(rpcError.id).toBe(1);
      expect(rpcError.error?.code).toBeTypeOf("number");
      expect(rpcError.error?.message).toBeTypeOf("string");

      await expectError(server, "/v1/plugins/missing-plugin", 404);
      await expectError(
        server,
        "/v1/plugins/missing-plugin/api-request",
        404,
        jsonInit(
          {
            method: "GET",
            path: "/v1/models"
          } satisfies Schema["PluginApiRequest"],
          { method: "POST" }
        )
      );
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

      await Promise.all(
        [
          "/v1/plugins/missing-plugin/enable",
          "/v1/plugins/missing-plugin/disable",
          "/v1/plugins/missing-plugin/start"
        ].map((path) => expectError(server, path, 404, { method: "POST" }))
      );

      await expectError(
        server,
        "/v1/plugins/missing-plugin/stop",
        404,
        jsonInit({ lastError: "smoke" } satisfies Schema["StopPluginRequest"], { method: "POST" })
      );
      await expectError(server, "/v1/plugins/missing-plugin", 404, { method: "DELETE" });
    });

    it.skipIf(Boolean(externalBaseUrl))(
      "imports and manages a generated plugin pack through its public lifecycle",
      async () => {
        const pluginId = `smoke-plugin-${Date.now()}`;
        const pack = await buildPluginPackFile(pluginId);
        const imported = await expectJson<Schema["PluginResponse"]>(
          server,
          "/v1/plugins/import-pack",
          {
            body: formDataWithFile(pack),
            method: "POST"
          }
        );
        expect(imported.response.ok).toBe(true);
        expect(imported.body).toMatchObject({
          enabled: true,
          id: pluginId,
          name: "Smoke Plugin",
          removable: true,
          runtimeStatus: "stopped",
          valid: true,
          version: "0.1.0"
        });

        try {
          const detail = await expectJson<Schema["PluginResponse"]>(
            server,
            `/v1/plugins/${pluginId}`
          );
          expect(detail.response.ok).toBe(true);
          expect(detail.body.id).toBe(pluginId);

          const disabled = await expectJson<Schema["PluginResponse"]>(
            server,
            `/v1/plugins/${pluginId}/disable`,
            { method: "POST" }
          );
          expect(disabled.body.enabled).toBe(false);
          expect(disabled.body.runtimeStatus).toBe("stopped");

          const enabled = await expectJson<Schema["PluginResponse"]>(
            server,
            `/v1/plugins/${pluginId}/enable`,
            { method: "POST" }
          );
          expect(enabled.body.enabled).toBe(true);
          expect(enabled.body.runtimeStatus).toBe("stopped");

          const started = await expectJson<Schema["PluginResponse"]>(
            server,
            `/v1/plugins/${pluginId}/start`,
            { method: "POST" }
          );
          expect(started.body.runtimeStatus).toBe("running");
          expect(started.body.lastStartedAt).toBeTypeOf("string");

          const stopped = await expectJson<Schema["PluginResponse"]>(
            server,
            `/v1/plugins/${pluginId}/stop`,
            jsonInit(
              { lastError: "smoke stopped" } satisfies Schema["StopPluginRequest"],
              { method: "POST" }
            )
          );
          expect(stopped.body.runtimeStatus).toBe("error");
          expect(stopped.body.lastError).toBe("smoke stopped");
          expect(stopped.body.lastStoppedAt).toBeTypeOf("string");

          const listed = await expectJson<Schema["PluginResponse"][]>(server, "/v1/plugins");
          expect(listed.body.some((plugin) => plugin.id === pluginId)).toBe(true);
        } finally {
          const deleted = await expectJson<Schema["DeletePluginResponse"]>(
            server,
            `/v1/plugins/${pluginId}`,
            { method: "DELETE" }
          );
          expect(deleted.body).toEqual({ deleted: true, id: pluginId });
        }

        await expectError(server, `/v1/plugins/${pluginId}`, 404);
      }
    );
  });
}
