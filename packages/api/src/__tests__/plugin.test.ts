import { describe, expect, it } from "vitest";

import {
  createSlabPluginApiFetch,
  type SlabApiBridgeRequest,
} from "../plugin";

describe("createSlabPluginApiFetch", () => {
  it("routes allowed API paths through the plugin bridge transport", async () => {
    let capturedRequest: SlabApiBridgeRequest | null = null;
    const pluginFetch = createSlabPluginApiFetch(
      async (request) => {
        capturedRequest = request;
        return {
          status: 200,
          headers: { "content-type": "application/json" },
          body: "{\"ok\":true}",
        };
      },
      { timeoutMs: 12_000 },
    );

    const response = await pluginFetch("/v1/models?capability=chat_generation", {
      headers: { "x-test": "1" },
    });

    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ ok: true });
    expect(capturedRequest).toEqual({
      method: "GET",
      path: "/v1/models?capability=chat_generation",
      headers: { "x-test": "1" },
      body: null,
      timeoutMs: 12_000,
    });
  });

  it("rejects API paths outside the declared plugin surface before invoking host IPC", async () => {
    let invoked = false;
    const pluginFetch = createSlabPluginApiFetch(async () => {
      invoked = true;
      throw new Error("should not be called");
    });

    await expect(pluginFetch("/v1/settings")).rejects.toThrow(
      "not part of the allowed plugin API surface",
    );
    expect(invoked).toBe(false);
  });
});
