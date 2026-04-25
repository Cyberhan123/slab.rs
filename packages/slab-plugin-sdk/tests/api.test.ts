import { describe, expect, it, vi } from "vitest";

import {
  createSlabPluginApiFetch,
  createSlabPluginSdk,
  SlabPluginApiError,
} from "../src/index";

type InvokeMock = (command: string, args?: unknown) => Promise<unknown>;

describe("plugin API bridge", () => {
  it("exposes low-level API fetch helpers from the SDK package", async () => {
    const pluginFetch = createSlabPluginApiFetch(async (request) => ({
      status: 200,
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ path: request.path }),
    }));

    const response = await pluginFetch("/v1/models?capability=chat_generation");

    await expect(response.json()).resolves.toEqual({
      path: "/v1/models?capability=chat_generation",
    });
  });

  it("exposes an OpenAPI client that routes through plugin_api_request", async () => {
    const invoke = vi.fn<InvokeMock>(async () => ({
      status: 200,
      headers: { "content-type": "application/json" },
      body: "[]",
    }));
    const sdk = createSlabPluginSdk(pluginWindow(invoke));

    const result = await sdk.api.client.GET("/v1/models", {
      params: { query: { capability: "chat_generation" } },
    });

    expect(result.data).toEqual([]);
    expect(invoke).toHaveBeenCalledWith("plugin_api_request", {
      request: {
        method: "GET",
        path: "/v1/models?capability=chat_generation",
        headers: {},
        body: null,
        timeoutMs: undefined,
      },
    });
  });

  it("keeps requestJson errors on the SDK error type", async () => {
    const invoke = vi.fn<InvokeMock>(async () => ({
      status: 403,
      headers: { "content-type": "application/json" },
      body: "{\"message\":\"missing permission\"}",
    }));
    const sdk = createSlabPluginSdk(pluginWindow(invoke));

    await expect(sdk.api.requestJson({ method: "GET", path: "/v1/models" })).rejects.toThrow(
      SlabPluginApiError,
    );
  });

  it("rejects requests outside the plugin API permission surface before IPC", async () => {
    const invoke = vi.fn<InvokeMock>();
    const sdk = createSlabPluginSdk(pluginWindow(invoke));

    await expect(sdk.api.requestJson({ method: "GET", path: "/v1/settings" })).rejects.toThrow(
      "not part of the allowed plugin API surface",
    );
    expect(invoke).not.toHaveBeenCalled();
  });
});

function pluginWindow(invoke: (command: string, args?: unknown) => Promise<unknown>): Window {
  return {
    __TAURI__: {
      core: {
        invoke,
      },
    },
  } as unknown as Window;
}
