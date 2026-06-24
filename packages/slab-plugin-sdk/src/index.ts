import { createSlabApiFetchClient } from "@slab/api";
import { SERVER_BASE_URL, normalizeApiBaseUrl } from "@slab/api/config";

import { assertSlabPluginApiSurface } from "./permissions";

export type {
  components as SlabApiComponents,
  operations as SlabApiOperations,
  paths as SlabApiPaths,
} from "@slab/api/v1";
export type {
  SlabApiPermission,
  SlabApiPermissionLabel,
  SlabApiPermissionSeverity,
} from "./permissions";
export {
  SLAB_API_PERMISSIONS,
  SLAB_API_PERMISSION_LABELS,
  describeSlabApiPermission,
  isKnownSlabApiPermission,
  requiredSlabApiPermission,
} from "./permissions";

/**
 * A plugin Slab API request expressed in transport-neutral terms. The SDK
 * serializes `body` to JSON (defaulting the content type) and resolves the
 * response as parsed JSON, throwing {@link SlabPluginApiError} on non-2xx.
 */
export type SlabPluginJsonRequest = {
  method: string;
  path: string;
  headers?: Record<string, string>;
  body?: unknown;
  timeoutMs?: number | null;
};

export type SlabPluginPickFileResponse = {
  path: string | null;
};

export type PluginUIHandle =
  | {
      kind: "tauri";
      pluginId: string;
      webviewLabel?: string;
      _targetWindow: Window;
    }
  | {
      kind: "browser";
      pluginId: string;
      iframe: HTMLIFrameElement;
    };

export type SlabPluginEventPayload = {
  pluginId: string;
  topic: string;
  data: unknown;
  ts: number;
};

export const SLAB_THEME_TOKENS = [
  "background",
  "foreground",
  "card",
  "card-foreground",
  "popover",
  "popover-foreground",
  "primary",
  "primary-foreground",
  "secondary",
  "secondary-foreground",
  "muted",
  "muted-foreground",
  "accent",
  "accent-foreground",
  "destructive",
  "destructive-foreground",
  "border",
  "input",
  "ring",
  "radius",
  "app-canvas",
  "surface-1",
  "surface-2",
  "surface-soft",
  "surface-selected",
  "surface-input",
  "brand-teal",
  "brand-teal-foreground",
  "brand-gold",
  "success",
  "success-foreground",
  "status-success-bg",
  "status-info-bg",
  "status-danger-bg",
  "status-neutral-bg",
] as const;

export type SlabThemeTokenName = (typeof SLAB_THEME_TOKENS)[number];
export type SlabThemeMode = "light" | "dark";

export type SlabThemeSnapshot = {
  mode: SlabThemeMode;
  tokens: Partial<Record<SlabThemeTokenName, string>>;
  updatedAt?: number;
};

type TauriEventApi = {
  listen: <T>(
    eventName: string,
    handler: (event: { payload: T }) => void,
  ) => Promise<() => void>;
};

type TauriCoreApi = {
  invoke<T>(command: string, args?: unknown): Promise<T>;
};

type TauriPluginWindow = Window & {
  __TAURI__?: {
    core?: TauriCoreApi;
    event?: TauriEventApi;
  };
};

const JSON_HEADERS = { "content-type": "application/json" };
const THEME_EVENT_NAME = "plugin://host/theme";

type SerializedApiRequest = {
  method: string;
  path: string;
  headers: Record<string, string>;
  body: string | null;
  timeoutMs?: number | null;
};

/**
 * Error thrown by {@link createSlabPluginSdk} API helpers when a plugin Slab API
 * request fails. `response` is the raw fetch `Response` and `data` is the parsed
 * body (JSON when parseable, otherwise the raw text).
 */
export class SlabPluginApiError extends Error {
  readonly response: Response;
  readonly data: unknown;

  constructor(message: string, response: Response, data: unknown) {
    super(message);
    this.name = "SlabPluginApiError";
    this.response = response;
    this.data = data;
  }
}

function resolveWindow(target?: Window): TauriPluginWindow {
  return (target ?? window) as TauriPluginWindow;
}

function requireCore(target?: Window): TauriCoreApi {
  const core = resolveWindow(target)["__TAURI__"]?.core;
  if (!core || typeof core.invoke !== "function") {
    throw new Error("Slab plugin host bridge is not available in this webview.");
  }
  return core;
}

function resolveCore(target?: Window): TauriCoreApi | null {
  const core = resolveWindow(target)["__TAURI__"]?.core;
  return core && typeof core.invoke === "function" ? core : null;
}

function resolveEventApi(target?: Window): TauriEventApi | null {
  const eventApi = resolveWindow(target)["__TAURI__"]?.event;
  return eventApi && typeof eventApi.listen === "function" ? eventApi : null;
}

function serializeJsonRequest(request: SlabPluginJsonRequest): SerializedApiRequest {
  const headers = { ...request.headers };
  let body: string | null = null;

  if (request.body !== undefined && request.body !== null) {
    body = typeof request.body === "string" ? request.body : JSON.stringify(request.body);
    const hasContentType = Object.keys(headers).some(
      (name) => name.toLowerCase() === "content-type",
    );
    if (!hasContentType) {
      headers["content-type"] = JSON_HEADERS["content-type"];
    }
  }

  return {
    method: request.method,
    path: request.path,
    headers,
    body,
    timeoutMs: request.timeoutMs,
  };
}

function fetchPluginApi(request: SerializedApiRequest): Promise<Response> {
  const endpoint = `${normalizeApiBaseUrl(SERVER_BASE_URL)}${request.path}`;
  const signal = request.timeoutMs ? AbortSignal.timeout(request.timeoutMs) : undefined;
  return fetch(endpoint, {
    method: request.method,
    headers: request.headers,
    body: request.body ?? undefined,
    signal,
  });
}

async function parseResponseBody(response: Response): Promise<unknown> {
  const text = await response.text();
  if (!text) {
    return null;
  }

  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

function extractErrorMessage(data: unknown): string | null {
  if (typeof data === "string" && data.trim()) {
    return data;
  }
  if (!data || typeof data !== "object") {
    return null;
  }

  const record = data as Record<string, unknown>;
  const nestedError = record.error;
  if (nestedError && typeof nestedError === "object") {
    const message = (nestedError as Record<string, unknown>).message;
    if (typeof message === "string" && message.trim()) {
      return message;
    }
  }
  if (typeof record.message === "string" && record.message.trim()) {
    return record.message;
  }
  return null;
}

export function applySlabThemeToDocument(
  snapshot: SlabThemeSnapshot,
  targetDocument: Document = document,
): void {
  const root = targetDocument.documentElement;
  root.classList.toggle("dark", snapshot.mode === "dark");

  for (const [token, value] of Object.entries(snapshot.tokens)) {
    if (typeof value === "string" && value.trim().length > 0) {
      root.style.setProperty(`--${token}`, value);
    }
  }
}

export type SlabPluginSdk = ReturnType<typeof createSlabPluginSdk>;
export type SlabPluginApiClient = ReturnType<typeof createSlabApiFetchClient>;

export function createSlabPluginSdk(target?: Window) {
  const apiClient = createSlabApiFetchClient({ baseUrl: SERVER_BASE_URL });
  // Enforce the plugin Slab API surface before any request leaves the webview.
  apiClient.use({
    async onRequest({ request }) {
      const url = new URL(request.url);
      assertSlabPluginApiSurface(request.method, `${url.pathname}${url.search}`);
      return request;
    },
  });

  return {
    host: {
      isAvailable: () => Boolean(resolveCore(target)),
      invoke: <T>(command: string, args?: unknown) => requireCore(target).invoke<T>(command, args),
    },
    api: {
      client: apiClient,
      requestJson: async <T>(request: SlabPluginJsonRequest): Promise<T> => {
        assertSlabPluginApiSurface(request.method, request.path);
        const response = await fetchPluginApi(serializeJsonRequest(request));
        const data = await parseResponseBody(response);
        if (!response.ok) {
          throw new SlabPluginApiError(
            extractErrorMessage(data) ?? `Plugin API request failed with HTTP ${response.status}`,
            response,
            data,
          );
        }
        return data as T;
      },
    },
    files: {
      pickVideo: () =>
        requireCore(target).invoke<SlabPluginPickFileResponse>("plugin_pick_file"),
    },
    events: {
      listen: async (
        pluginId: string,
        handler: (payload: SlabPluginEventPayload) => void,
      ): Promise<() => void> => {
        const eventApi = resolveEventApi(target);
        if (!eventApi) {
          return () => {};
        }
        return eventApi.listen<SlabPluginEventPayload>(
          `plugin://${pluginId}/event`,
          (event) => handler(event.payload),
        );
      },
    },
    theme: {
      getSnapshot: () =>
        requireCore(target).invoke<SlabThemeSnapshot>("plugin_theme_snapshot"),
      subscribe: async (handler: (snapshot: SlabThemeSnapshot) => void): Promise<() => void> => {
        const eventApi = resolveEventApi(target);
        if (!eventApi) {
          return () => {};
        }
        return eventApi.listen<SlabThemeSnapshot>(THEME_EVENT_NAME, (event) =>
          handler(event.payload),
        );
      },
      applyToDocument: (snapshot: SlabThemeSnapshot, targetDocument?: Document) => {
        const resolvedDocument = targetDocument ?? target?.document ?? document;
        applySlabThemeToDocument(snapshot, resolvedDocument);
      },
    },
  };
}

export function getSlabPluginSdk(target?: Window): SlabPluginSdk {
  return createSlabPluginSdk(target);
}

export function mountPluginUI(
  pluginId: string,
  entry: string,
  container: HTMLElement,
): PluginUIHandle {
  const targetWindow = resolveWindow(container.ownerDocument.defaultView ?? window);
  const tauriCore = targetWindow["__TAURI__"]?.core;
  const hasTrustedTauriContext = Boolean(
    (targetWindow as Window & { __TAURI_INTERNALS__?: unknown })["__TAURI_INTERNALS__"],
  );
  if (hasTrustedTauriContext && tauriCore && typeof tauriCore.invoke === "function") {
    const bounds = container.getBoundingClientRect();
    const handle: PluginUIHandle = { kind: "tauri", pluginId, _targetWindow: targetWindow };
    void tauriCore
      .invoke<{ webviewLabel: string }>("plugin_mount_view", {
        request: {
          pluginId,
          bounds: {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
          },
        },
      })
      .then((response) => {
        if (handle.kind === "tauri") {
          handle.webviewLabel = response.webviewLabel;
        }
      });
    return handle;
  }

  // eslint-disable-next-line react/iframe-missing-sandbox -- sandbox is set before the iframe is mounted.
  const iframe = container.ownerDocument.createElement("iframe");
  iframe.setAttribute("sandbox", "allow-scripts allow-forms");
  iframe.src = entry;
  iframe.style.width = "100%";
  iframe.style.height = "100%";
  iframe.style.border = "0";
  container.appendChild(iframe);
  return { kind: "browser", pluginId, iframe };
}

export function unmountPluginUI(handle: PluginUIHandle): void {
  if (handle.kind === "tauri") {
    // eslint-disable-next-line no-underscore-dangle -- Preserve the existing exported PluginUIHandle field name.
    const targetWindow = handle._targetWindow as TauriPluginWindow;
    const tauriCore = targetWindow["__TAURI__"]?.core;
    const hasTrustedTauriContext = Boolean(
      (targetWindow as Window & { __TAURI_INTERNALS__?: unknown })["__TAURI_INTERNALS__"],
    );
    if (hasTrustedTauriContext && tauriCore && typeof tauriCore.invoke === "function") {
      void tauriCore.invoke("plugin_unmount_view", { request: { pluginId: handle.pluginId } });
    }
    return;
  }

  handle.iframe.remove();
}
