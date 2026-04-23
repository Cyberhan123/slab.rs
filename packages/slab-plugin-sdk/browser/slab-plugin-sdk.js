(() => {
  var __defProp = Object.defineProperty;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  var __moduleCache = /* @__PURE__ */ new WeakMap;
  var __toCommonJS = (from) => {
    var entry = __moduleCache.get(from), desc;
    if (entry)
      return entry;
    entry = __defProp({}, "__esModule", { value: true });
    if (from && typeof from === "object" || typeof from === "function")
      __getOwnPropNames(from).map((key) => !__hasOwnProp.call(entry, key) && __defProp(entry, key, {
        get: () => from[key],
        enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
      }));
    __moduleCache.set(from, entry);
    return entry;
  };
  var __export = (target, all) => {
    for (var name in all)
      __defProp(target, name, {
        get: all[name],
        enumerable: true,
        configurable: true,
        set: (newValue) => all[name] = () => newValue
      });
  };

  // src/index.ts
  var exports_src = {};
  __export(exports_src, {
    getSlabPluginSdk: () => getSlabPluginSdk,
    createSlabPluginSdk: () => createSlabPluginSdk,
    applySlabThemeToDocument: () => applySlabThemeToDocument,
    SlabPluginApiError: () => SlabPluginApiError,
    SLAB_THEME_TOKENS: () => SLAB_THEME_TOKENS
  });
  var SLAB_THEME_TOKENS = [
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
    "status-neutral-bg"
  ];
  var JSON_HEADERS = { "content-type": "application/json" };
  var THEME_EVENT_NAME = "plugin://host/theme";

  class SlabPluginApiError extends Error {
    response;
    data;
    constructor(message, response, data) {
      super(message);
      this.name = "SlabPluginApiError";
      this.response = response;
      this.data = data;
    }
  }
  function resolveWindow(target) {
    return target ?? window;
  }
  function requireCore(target) {
    const core = resolveWindow(target).__TAURI__?.core;
    if (!core || typeof core.invoke !== "function") {
      throw new Error("Slab plugin host bridge is not available in this webview.");
    }
    return core;
  }
  function resolveEventApi(target) {
    const eventApi = resolveWindow(target).__TAURI__?.event;
    return eventApi && typeof eventApi.listen === "function" ? eventApi : null;
  }
  function serializeJsonRequest(request) {
    const headers = { ...request.headers };
    let body = null;
    if (request.body !== undefined && request.body !== null) {
      body = typeof request.body === "string" ? request.body : JSON.stringify(request.body);
      const hasContentType = Object.keys(headers).some((name) => name.toLowerCase() === "content-type");
      if (!hasContentType) {
        headers["content-type"] = JSON_HEADERS["content-type"];
      }
    }
    return {
      method: request.method,
      path: request.path,
      headers,
      body,
      timeoutMs: request.timeoutMs
    };
  }
  function parseResponseBody(response) {
    if (!response.body) {
      return null;
    }
    try {
      return JSON.parse(response.body);
    } catch {
      return response.body;
    }
  }
  function extractErrorMessage(data) {
    if (typeof data === "string" && data.trim()) {
      return data;
    }
    if (!data || typeof data !== "object") {
      return null;
    }
    const record = data;
    const nestedError = record.error;
    if (nestedError && typeof nestedError === "object") {
      const message = nestedError.message;
      if (typeof message === "string" && message.trim()) {
        return message;
      }
    }
    if (typeof record.message === "string" && record.message.trim()) {
      return record.message;
    }
    return null;
  }
  function applySlabThemeToDocument(snapshot, targetDocument = document) {
    const root = targetDocument.documentElement;
    root.classList.toggle("dark", snapshot.mode === "dark");
    for (const [token, value] of Object.entries(snapshot.tokens)) {
      if (typeof value === "string" && value.trim().length > 0) {
        root.style.setProperty(`--${token}`, value);
      }
    }
  }
  function createSlabPluginSdk(target) {
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
        invoke: (command, args) => requireCore(target).invoke(command, args)
      },
      api: {
        request: (request) => requireCore(target).invoke("plugin_api_request", { request }),
        requestJson: async (request) => {
          const response = await requireCore(target).invoke("plugin_api_request", { request: serializeJsonRequest(request) });
          const data = parseResponseBody(response);
          if (response.status < 200 || response.status >= 300) {
            throw new SlabPluginApiError(extractErrorMessage(data) ?? `Plugin API request failed with HTTP ${response.status}`, response, data);
          }
          return data;
        }
      },
      files: {
        pickVideo: () => requireCore(target).invoke("plugin_pick_file")
      },
      events: {
        listen: async (pluginId, handler) => {
          const eventApi = resolveEventApi(target);
          if (!eventApi) {
            return () => {};
          }
          return eventApi.listen(`plugin://${pluginId}/event`, (event) => handler(event.payload));
        }
      },
      theme: {
        getSnapshot: () => requireCore(target).invoke("plugin_theme_snapshot"),
        subscribe: async (handler) => {
          const eventApi = resolveEventApi(target);
          if (!eventApi) {
            return () => {};
          }
          return eventApi.listen(THEME_EVENT_NAME, (event) => handler(event.payload));
        },
        applyToDocument: (snapshot, targetDocument) => {
          const resolvedDocument = targetDocument ?? target?.document ?? document;
          applySlabThemeToDocument(snapshot, resolvedDocument);
        }
      }
    };
  }
  function getSlabPluginSdk(target) {
    return createSlabPluginSdk(target);
  }
})();
