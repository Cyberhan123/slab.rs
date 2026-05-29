import { beforeAll, describe, expect, it } from "vitest";

import type { SlabServerTestHarness } from "../../support/slab-server";
import { expectJson, jsonInit, type Schema } from "./shared";

export function registerSessionsAndUiStateSmoke(
  getServer: () => SlabServerTestHarness
): void {
  describe("slab-server smoke sessions and ui state", () => {
    let server: SlabServerTestHarness;

    beforeAll(() => {
      server = getServer();
    });

    it("covers sessions and UI state persistence routes", async () => {
      const created = await expectJson<Schema["SessionResponse"]>(
        server,
        "/v1/sessions",
        jsonInit({ name: "Vitest smoke session" } satisfies Schema["CreateSessionRequest"], {
          method: "POST"
        })
      );
      expect(created.response.ok).toBe(true);
      expect(created.body.name).toBe("Vitest smoke session");

      const renamed = await expectJson<Schema["SessionResponse"]>(
        server,
        `/v1/sessions/${created.body.id}`,
        jsonInit({ name: "Renamed smoke session" } satisfies Schema["UpdateSessionRequest"], {
          method: "PUT"
        })
      );
      expect(renamed.response.ok).toBe(true);
      expect(renamed.body.name).toBe("Renamed smoke session");

      const listed = await expectJson<Schema["SessionResponse"][]>(server, "/v1/sessions");
      expect(
        listed.body.some(
          (session) => session.id === created.body.id && session.name === "Renamed smoke session"
        )
      ).toBe(true);

      const messages = await expectJson<Schema["MessageResponse"][]>(
        server,
        `/v1/sessions/${created.body.id}/messages`
      );
      expect(messages.response.ok).toBe(true);
      expect(Array.isArray(messages.body)).toBe(true);

      const deleted = await expectJson<unknown>(server, `/v1/sessions/${created.body.id}`, {
        method: "DELETE"
      });
      expect(deleted.response.ok).toBe(true);

      const key = `smoke.${Date.now()}`;
      const updatedState = await expectJson<Schema["UiStateValueResponse"]>(
        server,
        `/v1/ui-state/${encodeURIComponent(key)}`,
        jsonInit({ value: "ready" } satisfies Schema["UpdateUiStateRequest"], { method: "PUT" })
      );
      expect(updatedState.body.key).toBe(key);
      expect(updatedState.body.value).toBe("ready");

      const fetchedState = await expectJson<Schema["UiStateValueResponse"]>(
        server,
        `/v1/ui-state/${encodeURIComponent(key)}`
      );
      expect(fetchedState.body.value).toBe("ready");

      const deletedState = await expectJson<Schema["UiStateDeleteResponse"]>(
        server,
        `/v1/ui-state/${encodeURIComponent(key)}`,
        { method: "DELETE" }
      );
      expect(deletedState.body.deleted).toBe(true);
    });
  });
}
