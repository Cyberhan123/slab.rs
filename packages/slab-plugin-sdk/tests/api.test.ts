import { describe, expect, it, vi } from "vitest";

import {
  createSlabPluginApiFetch,
  createSlabPluginSdk,
  getSlabPluginSdk,
  mountPluginUI,
  SlabPluginApiError,
  unmountPluginUI,
} from "../src/index";

type InvokeMock = (command: string, args?: unknown) => Promise<unknown>;
type UnlistenMock = () => void;
type PluginEventHandler = (event: { payload: unknown }) => void;
type ListenMock = (eventName: string, handler: PluginEventHandler) => Promise<UnlistenMock>;

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

  it("serializes JSON request bodies and preserves explicit content type headers", async () => {
    const invoke = vi.fn<InvokeMock>(async () => ({
      status: 200,
      headers: { "content-type": "application/json" },
      body: "{\"ok\":true}",
    }));
    const sdk = createSlabPluginSdk(pluginWindow(invoke));

    await expect(
      sdk.api.requestJson({
        method: "POST",
        path: "/v1/models/load",
        headers: { "Content-Type": "application/vnd.slab+json" },
        body: { type: "model.download" },
        timeoutMs: 500,
      }),
    ).resolves.toEqual({ ok: true });

    expect(invoke).toHaveBeenCalledWith("plugin_api_request", {
      request: {
        method: "POST",
        path: "/v1/models/load",
        headers: { "Content-Type": "application/vnd.slab+json" },
        body: "{\"type\":\"model.download\"}",
        timeoutMs: 500,
      },
    });
  });

  it("uses nested API error messages and keeps parsed error data", async () => {
    const invoke = vi.fn<InvokeMock>(async () => ({
      status: 500,
      headers: { "content-type": "application/json" },
      body: "{\"error\":{\"message\":\"runtime offline\"}}",
    }));
    const sdk = createSlabPluginSdk(pluginWindow(invoke));

    await expect(sdk.api.requestJson({ method: "GET", path: "/v1/models" })).rejects.toMatchObject({
      data: { error: { message: "runtime offline" } },
      message: "runtime offline",
      name: "SlabPluginApiError",
      response: {
        status: 500,
      },
    });
  });

  it("falls back to raw response bodies and HTTP status for requestJson errors", async () => {
    const sdkWithTextError = createSlabPluginSdk(
      pluginWindow(async () => ({
        status: 502,
        headers: {},
        body: "bad gateway",
      })),
    );
    const sdkWithEmptyError = createSlabPluginSdk(
      pluginWindow(async () => ({
        status: 418,
        headers: {},
        body: "",
      })),
    );

    await expect(
      sdkWithTextError.api.requestJson({ method: "GET", path: "/v1/models" }),
    ).rejects.toThrow("bad gateway");
    await expect(
      sdkWithEmptyError.api.requestJson({ method: "GET", path: "/v1/models" }),
    ).rejects.toThrow("Plugin API request failed with HTTP 418");
  });

  it("rejects requests outside the plugin API permission surface before IPC", async () => {
    const invoke = vi.fn<InvokeMock>();
    const sdk = createSlabPluginSdk(pluginWindow(invoke));

    await expect(sdk.api.requestJson({ method: "GET", path: "/v1/settings" })).rejects.toThrow(
      "not part of the allowed plugin API surface",
    );
    expect(invoke).not.toHaveBeenCalled();
  });

  it("reports host availability and proxies host commands", async () => {
    const invoke = vi.fn<InvokeMock>(async () => "ok");
    const sdk = getSlabPluginSdk(pluginWindow(invoke));
    const unavailableSdk = createSlabPluginSdk({} as Window);

    expect(sdk.host.isAvailable()).toBe(true);
    await expect(sdk.host.invoke("plugin_custom_command", { value: 1 })).resolves.toBe("ok");
    expect(invoke).toHaveBeenCalledWith("plugin_custom_command", { value: 1 });
    expect(unavailableSdk.host.isAvailable()).toBe(false);
    expect(() => unavailableSdk.host.invoke("plugin_custom_command")).toThrow(
      "Slab plugin host bridge is not available in this webview.",
    );
  });

  it("proxies file picking, plugin events, and theme subscription through Tauri APIs", async () => {
    const invoke = vi.fn<InvokeMock>(async () => ({ path: "video.mp4" }));
    const unlisten = vi.fn<UnlistenMock>();
    const listen = vi.fn<ListenMock>(
      async (_eventName: string, handler: PluginEventHandler) => {
        handler({ payload: { pluginId: "demo", topic: "ready", data: null, ts: 1 } });
        return unlisten;
      },
    );
    const sdk = createSlabPluginSdk(pluginWindow(invoke, listen));
    const eventHandler = vi.fn<(payload: unknown) => void>();
    const themeHandler = vi.fn<(payload: unknown) => void>();

    await expect(sdk.files.pickVideo()).resolves.toEqual({ path: "video.mp4" });
    await expect(sdk.events.listen("demo", eventHandler)).resolves.toBe(unlisten);
    await expect(sdk.theme.subscribe(themeHandler)).resolves.toBe(unlisten);
    await expect(sdk.theme.getSnapshot()).resolves.toEqual({ path: "video.mp4" });

    expect(listen).toHaveBeenCalledWith("plugin://demo/event", expect.any(Function));
    expect(listen).toHaveBeenCalledWith("plugin://host/theme", expect.any(Function));
    expect(eventHandler).toHaveBeenCalledWith({
      data: null,
      pluginId: "demo",
      topic: "ready",
      ts: 1,
    });
    expect(themeHandler).toHaveBeenCalledWith({
      data: null,
      pluginId: "demo",
      topic: "ready",
      ts: 1,
    });
  });

  it("uses noop event unsubscribers when the event bridge is unavailable", async () => {
    const sdk = createSlabPluginSdk(
      pluginWindow(async () => ({ status: 200, headers: {}, body: null })),
    );

    expect(await sdk.events.listen("demo", vi.fn<(payload: unknown) => void>())).toEqual(
      expect.any(Function),
    );
    expect(await sdk.theme.subscribe(vi.fn<(payload: unknown) => void>())).toEqual(
      expect.any(Function),
    );
  });
});

describe("plugin UI mounting", () => {
  it("mounts browser iframe plugin UIs with a sandbox", () => {
    const container = document.createElement("div");

    const handle = mountPluginUI("demo", "https://example.test/plugin", container);

    expect(handle.kind).toBe("browser");
    expect(container.querySelector("iframe")).toMatchObject({
      src: "https://example.test/plugin",
    });
    expect(container.querySelector("iframe")?.getAttribute("sandbox")).toBe(
      "allow-scripts allow-forms",
    );

    unmountPluginUI(handle);

    expect(container.querySelector("iframe")).toBeNull();
  });

  it("mounts and unmounts trusted Tauri plugin webviews", async () => {
    const invoke = vi.fn<InvokeMock>(async (command) => {
      if (command === "plugin_mount_view") {
        return { webviewLabel: "plugin-demo" };
      }
      return null;
    });
    const hostDocument = window.document.implementation.createHTMLDocument("host");
    const targetWindow = pluginWindow(invoke);
    Object.defineProperty(targetWindow, "__TAURI_INTERNALS__", { value: {} });
    Object.defineProperty(hostDocument, "defaultView", { value: targetWindow });
    const container = hostDocument.createElement("div");
    container.getBoundingClientRect = () =>
      ({
        x: 1,
        y: 2,
        width: 300,
        height: 200,
      }) as DOMRect;

    const handle = mountPluginUI("demo", "https://example.test/plugin", container);
    await new Promise((resolve) => window.setTimeout(resolve, 0));

    expect(handle).toMatchObject({
      kind: "tauri",
      pluginId: "demo",
      webviewLabel: "plugin-demo",
    });
    expect(invoke).toHaveBeenCalledWith("plugin_mount_view", {
      request: {
        pluginId: "demo",
        bounds: {
          height: 200,
          width: 300,
          x: 1,
          y: 2,
        },
      },
    });

    unmountPluginUI(handle);

    expect(invoke).toHaveBeenCalledWith("plugin_unmount_view", {
      request: { pluginId: "demo" },
    });
  });
});

function pluginWindow(
  invoke: (command: string, args?: unknown) => Promise<unknown>,
  listen?: (eventName: string, handler: (event: { payload: unknown }) => void) => Promise<() => void>,
): Window {
  return {
    __TAURI__: {
      core: {
        invoke,
      },
      event: listen ? { listen } : undefined,
    },
  } as unknown as Window;
}
