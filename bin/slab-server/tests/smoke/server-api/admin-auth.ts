import { afterAll, beforeAll, describe, expect, it } from "vitest";

import {
  startSlabServerHarness,
  type SlabServerTestHarness
} from "../../support/slab-server";
import { externalBaseUrl, expectJson, type Schema } from "./shared";

export function registerAdminAuthSmoke(): void {
  describe("slab-server admin authentication smoke", () => {
    let server: SlabServerTestHarness | undefined;

    beforeAll(async () => {
      if (externalBaseUrl) {
        return;
      }

      server = await startSlabServerHarness({
        adminToken: "vitest-admin-token"
      });
    });

    afterAll(async () => {
      await server?.stop();
    });

    it.skipIf(Boolean(externalBaseUrl))("requires bearer auth for management routes", async () => {
      const settings = await server!.request("/v1/settings");
      expect(settings.status).toBe(401);

      const backends = await server!.request("/v1/backends");
      expect(backends.status).toBe(401);

      const authorized = await expectJson<Schema["SettingsDocumentView"]>(server!, "/v1/settings", {
        headers: {
          Authorization: "Bearer vitest-admin-token"
        }
      });
      expect(authorized.response.ok).toBe(true);
      expect(Array.isArray(authorized.body.sections)).toBe(true);
    });
  });
}
