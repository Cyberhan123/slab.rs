import { assertSlabPluginApiSurface } from "@slab/api/permissions";
import {
  createSlabPluginApiClient,
  createSlabPluginApiFetch,
  type SlabApiBridgeRequest,
  type SlabApiBridgeResponse,
  type SlabApiBridgeTransport,
  type SlabApiFetch,
  type SlabPluginApiClient,
} from "@slab/api/plugin";

export type {
  components as SlabApiComponents,
  operations as SlabApiOperations,
  paths as SlabApiPaths,
} from "@slab/api/v1";
export type { SlabApiPermission } from "@slab/api/permissions";

export type SlabPluginApiRequest = SlabApiBridgeRequest;
export type SlabPluginApiResponse = SlabApiBridgeResponse;
export type SlabPluginApiTransport = SlabApiBridgeTransport;
export type SlabPluginApiFetch = SlabApiFetch;
export type SlabPluginOpenApiClient = SlabPluginApiClient;

export { createSlabPluginApiClient, createSlabPluginApiFetch };

export type SlabPluginJsonRequest = Omit<SlabPluginApiRequest, "body" | "headers"> & {
  headers?: Record<string, string>;
  body?: unknown;
};

export type SlabPluginPickFileResponse = {
  path: string | null;
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

export class SlabPluginApiError extends Error {
  readonly response: SlabPluginApiResponse;
  readonly data: unknown;

  constructor(message: string, response: SlabPluginApiResponse, data: unknown) {
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
  const core = resolveWindow(target).__TAURI__?.core;
  if (!core || typeof core.invoke !== "function") {
    throw new Error("Slab plugin host bridge is not available in this webview.");
  }
  return core;
}

function resolveEventApi(target?: Window): TauriEventApi | null {
  const eventApi = resolveWindow(target).__TAURI__?.event;
  return eventApi && typeof eventApi.listen === "function" ? eventApi : null;
}

function serializeJsonRequest(request: SlabPluginJsonRequest): SlabPluginApiRequest {
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

function parseResponseBody(response: SlabPluginApiResponse): unknown {
  if (!response.body) {
    return null;
  }

  try {
    return JSON.parse(response.body);
  } catch {
    return response.body;
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

export function createSlabPluginSdk(target?: Window) {
  const invokeApiRequest = (request: SlabPluginApiRequest) => {
    assertSlabPluginApiSurface(request.method, request.path);
    return requireCore(target).invoke<SlabPluginApiResponse>("plugin_api_request", { request });
  };
  const apiFetch = createSlabPluginApiFetch(invokeApiRequest);
  const apiClient = createSlabPluginApiClient(invokeApiRequest);

  return {
    host: {
      isAvailable: () => {
        try {
          requireCore(target);
          return true;
        } catch {
          return false;
        }
      },
      invoke: <T>(command: string, args?: unknown) => requireCore(target).invoke<T>(command, args),
    },
    api: {
      client: apiClient,
      fetch: apiFetch,
      request: invokeApiRequest,
      requestJson: async <T>(request: SlabPluginJsonRequest): Promise<T> => {
        const response = await invokeApiRequest(serializeJsonRequest(request));
        const data = parseResponseBody(response);
        if (response.status < 200 || response.status >= 300) {
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
