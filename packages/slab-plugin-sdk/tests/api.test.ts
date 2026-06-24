import { afterEach, describe, expect, it, vi } from "vitest";

import {
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

afterEach(() => {
  vi.unstubAllGlobals();
});

function jsonResponse(body: unknown, status = 200): Response {
  const text =
    body === null || body === undefined
      ? null
      : typeof body === "string"
        ? body
        : JSON.stringify(body);
  return new Response(text, {
    status,
    headers: { "content-type": "application/json" },
  });
}

describe("plugin API client", () => {
  it("routes on-surface requestJson to slab-server over fetch", async () => {
    const fetchMock = vi.fn<() => Promise<Response>>(() => Promise.resolve(jsonResponse([])));
    vi.stubGlobal("fetch", fetchMock);
    const sdk = createSlabPluginSdk();

    const result = await sdk.api.requestJson<unknown[]>({ method: "GET", path: "/v1/models" });

    expect(result).toEqual([]);
    expect(fetchMock).toHaveBeenCalledWith(
      "http://127.0.0.1:3000/v1/models",
      expect.objectContaining({ method: "GET" }),
    );
  });

  it("rejects off-surface requestJson before invoking fetch", async () => {
    const fetchMock = vi.fn<() => Promise<Response>>(() => Promise.reject(new Error("fetch must not be called")));
    vi.stubGlobal("fetch", fetchMock);
    const sdk = createSlabPluginSdk();

    await expect(
      sdk.api.requestJson({ method: "GET", path: "/v1/settings" }),
    ).rejects.toThrow("not part of the allowed plugin API surface");
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("keeps requestJson errors on the SDK error type", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn<() => Promise<Response>>(() => Promise.resolve(jsonResponse({ message: "missing permission" }, 403))),
    );
    const sdk = createSlabPluginSdk();

    await expect(sdk.api.requestJson({ method: "GET", path: "/v1/models" })).rejects.toThrow(
      SlabPluginApiError,
    );
  });

  it("serializes JSON request bodies and preserves explicit content type headers", async () => {
    const fetchMock = vi.fn<() => Promise<Response>>(() => Promise.resolve(jsonResponse({ ok: true })));
    vi.stubGlobal("fetch", fetchMock);
    const sdk = createSlabPluginSdk();

    await expect(
      sdk.api.requestJson({
        method: "POST",
        path: "/v1/models/load",
        headers: { "Content-Type": "application/vnd.slab+json" },
        body: { type: "model.download" },
        timeoutMs: 500,
      }),
    ).resolves.toEqual({ ok: true });

    expect(fetchMock).toHaveBeenCalledWith(
      "http://127.0.0.1:3000/v1/models/load",
      expect.objectContaining({
        method: "POST",
        headers: { "Content-Type": "application/vnd.slab+json" },
        body: JSON.stringify({ type: "model.download" }),
      }),
    );
  });

  it("uses nested API error messages and keeps parsed error data", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(jsonResponse({ error: { message: "runtime offline" } }, 500)),
      ),
    );
    const sdk = createSlabPluginSdk();

    await expect(sdk.api.requestJson({ method: "GET", path: "/v1/models" })).rejects.toMatchObject({
      data: { error: { message: "runtime offline" } },
      message: "runtime offline",
      name: "SlabPluginApiError",
      response: { status: 500 },
    });
  });

  it("falls back to raw response bodies and HTTP status for requestJson errors", async () => {
    vi.stubGlobal("fetch", vi.fn<() => Promise<Response>>(() => Promise.resolve(jsonResponse("bad gateway", 502))));
    const sdkWithTextError = createSlabPluginSdk();

    await expect(
      sdkWithTextError.api.requestJson({ method: "GET", path: "/v1/models" }),
    ).rejects.toThrow("bad gateway");

    vi.stubGlobal("fetch", vi.fn<() => Promise<Response>>(() => Promise.resolve(jsonResponse(null, 418))));
    const sdkWithEmptyError = createSlabPluginSdk();

    await expect(
      sdkWithEmptyError.api.requestJson({ method: "GET", path: "/v1/models" }),
    ).rejects.toThrow("Plugin API request failed with HTTP 418");
  });

  it("routes the OpenAPI client through fetch and parses the response", async () => {
    // Type the mock with the fetch call signature so the recorded call args are
    // reachable; openapi-fetch issues a Request object as the single argument.
    const fetchMock = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) =>
      jsonResponse([]),
    );
    vi.stubGlobal("fetch", fetchMock);
    const sdk = createSlabPluginSdk();

    const result = await sdk.api.client.GET("/v1/models", {
      params: { query: { capability: "chat_generation" } },
    });

    expect(result.data).toEqual([]);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    // openapi-fetch issues a Request object, not a (url, init) pair.
    const request = fetchMock.mock.calls[0][0] as Request;
    expect(request.method).toBe("GET");
    expect(request.url).toBe("http://127.0.0.1:3000/v1/models?capability=chat_generation");
  });

  it("rejects off-surface OpenAPI client requests before invoking fetch", async () => {
    const fetchMock = vi.fn<() => Promise<Response>>(() => Promise.reject(new Error("fetch must not be called")));
    vi.stubGlobal("fetch", fetchMock);
    const sdk = createSlabPluginSdk();

    await expect(sdk.api.client.GET("/v1/workspace")).rejects.toThrow(
      "not part of the allowed plugin API surface",
    );
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

describe("plugin host bridge", () => {
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
    const sdk = createSlabPluginSdk(pluginWindow(vi.fn()));

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
