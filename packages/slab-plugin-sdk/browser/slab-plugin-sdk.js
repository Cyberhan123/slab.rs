(() => {
  var __defProp = Object.defineProperty;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  function __accessProp(key) {
    return this[key];
  }
  var __toCommonJS = (from) => {
    var entry = (__moduleCache ??= new WeakMap).get(from), desc;
    if (entry)
      return entry;
    entry = __defProp({}, "__esModule", { value: true });
    if (from && typeof from === "object" || typeof from === "function") {
      for (var key of __getOwnPropNames(from))
        if (!__hasOwnProp.call(entry, key))
          __defProp(entry, key, {
            get: __accessProp.bind(from, key),
            enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
          });
    }
    __moduleCache.set(from, entry);
    return entry;
  };
  var __moduleCache;
  var __returnValue = (v) => v;
  function __exportSetter(name, newValue) {
    this[name] = __returnValue.bind(null, newValue);
  }
  var __export = (target, all) => {
    for (var name in all)
      __defProp(target, name, {
        get: all[name],
        enumerable: true,
        configurable: true,
        set: __exportSetter.bind(all, name)
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

  // ../api/src/permissions.ts
  var SLAB_API_PERMISSIONS = {
    modelsRead: "models:read",
    ffmpegConvert: "ffmpeg:convert",
    audioTranscribe: "audio:transcribe",
    subtitleRender: "subtitle:render",
    chatComplete: "chat:complete",
    tasksRead: "tasks:read",
    tasksCancel: "tasks:cancel"
  };
  function requiredSlabApiPermission(method, path) {
    const normalizedMethod = method.toUpperCase();
    const normalizedPath = path.split("?").at(0) ?? path;
    switch (normalizedMethod) {
      case "GET":
        if (pathMatches(normalizedPath, "/v1/models")) {
          return SLAB_API_PERMISSIONS.modelsRead;
        }
        if (pathMatches(normalizedPath, "/v1/tasks")) {
          return SLAB_API_PERMISSIONS.tasksRead;
        }
        return null;
      case "POST":
        if (normalizedPath === "/v1/ffmpeg/convert") {
          return SLAB_API_PERMISSIONS.ffmpegConvert;
        }
        if (normalizedPath === "/v1/audio/transcriptions") {
          return SLAB_API_PERMISSIONS.audioTranscribe;
        }
        if (normalizedPath === "/v1/subtitles/render") {
          return SLAB_API_PERMISSIONS.subtitleRender;
        }
        if (normalizedPath === "/v1/chat/completions") {
          return SLAB_API_PERMISSIONS.chatComplete;
        }
        if (normalizedPath.startsWith("/v1/tasks/") && normalizedPath.endsWith("/cancel")) {
          return SLAB_API_PERMISSIONS.tasksCancel;
        }
        return null;
      default:
        return null;
    }
  }
  function assertSlabPluginApiSurface(method, path) {
    const requiredPermission = requiredSlabApiPermission(method, path);
    if (requiredPermission) {
      return requiredPermission;
    }
    throw new Error(`Plugin API request ${method.toUpperCase()} ${path} is not part of the allowed plugin API surface.`);
  }
  function pathMatches(path, base) {
    return path === base || path.startsWith(`${base}/`);
  }

  // ../../node_modules/openapi-fetch/dist/index.mjs
  var PATH_PARAM_RE = /\{[^{}]+\}/g;
  var supportsRequestInitExt = () => {
    return typeof process === "object" && Number.parseInt(process?.versions?.node?.substring(0, 2)) >= 18 && process.versions.undici;
  };
  function randomID() {
    return Math.random().toString(36).slice(2, 11);
  }
  function createClient(clientOptions) {
    let {
      baseUrl = "",
      Request: CustomRequest = globalThis.Request,
      fetch: baseFetch = globalThis.fetch,
      querySerializer: globalQuerySerializer,
      bodySerializer: globalBodySerializer,
      pathSerializer: globalPathSerializer,
      headers: baseHeaders,
      requestInitExt = undefined,
      ...baseOptions
    } = { ...clientOptions };
    requestInitExt = supportsRequestInitExt() ? requestInitExt : undefined;
    baseUrl = removeTrailingSlash(baseUrl);
    const globalMiddlewares = [];
    async function coreFetch(schemaPath, fetchOptions) {
      const {
        baseUrl: localBaseUrl,
        fetch = baseFetch,
        Request: Request2 = CustomRequest,
        headers,
        params = {},
        parseAs = "json",
        querySerializer: requestQuerySerializer,
        bodySerializer = globalBodySerializer ?? defaultBodySerializer,
        pathSerializer: requestPathSerializer,
        body,
        middleware: requestMiddlewares = [],
        ...init
      } = fetchOptions || {};
      let finalBaseUrl = baseUrl;
      if (localBaseUrl) {
        finalBaseUrl = removeTrailingSlash(localBaseUrl) ?? baseUrl;
      }
      let querySerializer = typeof globalQuerySerializer === "function" ? globalQuerySerializer : createQuerySerializer(globalQuerySerializer);
      if (requestQuerySerializer) {
        querySerializer = typeof requestQuerySerializer === "function" ? requestQuerySerializer : createQuerySerializer({
          ...typeof globalQuerySerializer === "object" ? globalQuerySerializer : {},
          ...requestQuerySerializer
        });
      }
      const pathSerializer = requestPathSerializer || globalPathSerializer || defaultPathSerializer;
      const serializedBody = body === undefined ? undefined : bodySerializer(body, mergeHeaders(baseHeaders, headers, params.header));
      const finalHeaders = mergeHeaders(serializedBody === undefined || serializedBody instanceof FormData ? {} : {
        "Content-Type": "application/json"
      }, baseHeaders, headers, params.header);
      const finalMiddlewares = [...globalMiddlewares, ...requestMiddlewares];
      const requestInit = {
        redirect: "follow",
        ...baseOptions,
        ...init,
        body: serializedBody,
        headers: finalHeaders
      };
      let id;
      let options;
      let request = new Request2(createFinalURL(schemaPath, { baseUrl: finalBaseUrl, params, querySerializer, pathSerializer }), requestInit);
      let response;
      for (const key in init) {
        if (!(key in request)) {
          request[key] = init[key];
        }
      }
      if (finalMiddlewares.length) {
        id = randomID();
        options = Object.freeze({
          baseUrl: finalBaseUrl,
          fetch,
          parseAs,
          querySerializer,
          bodySerializer,
          pathSerializer
        });
        for (const m of finalMiddlewares) {
          if (m && typeof m === "object" && typeof m.onRequest === "function") {
            const result = await m.onRequest({
              request,
              schemaPath,
              params,
              options,
              id
            });
            if (result) {
              if (result instanceof Request2) {
                request = result;
              } else if (result instanceof Response) {
                response = result;
                break;
              } else {
                throw new Error("onRequest: must return new Request() or Response() when modifying the request");
              }
            }
          }
        }
      }
      if (!response) {
        try {
          response = await fetch(request, requestInitExt);
        } catch (error2) {
          let errorAfterMiddleware = error2;
          if (finalMiddlewares.length) {
            for (let i = finalMiddlewares.length - 1;i >= 0; i--) {
              const m = finalMiddlewares[i];
              if (m && typeof m === "object" && typeof m.onError === "function") {
                const result = await m.onError({
                  request,
                  error: errorAfterMiddleware,
                  schemaPath,
                  params,
                  options,
                  id
                });
                if (result) {
                  if (result instanceof Response) {
                    errorAfterMiddleware = undefined;
                    response = result;
                    break;
                  }
                  if (result instanceof Error) {
                    errorAfterMiddleware = result;
                    continue;
                  }
                  throw new Error("onError: must return new Response() or instance of Error");
                }
              }
            }
          }
          if (errorAfterMiddleware) {
            throw errorAfterMiddleware;
          }
        }
        if (finalMiddlewares.length) {
          for (let i = finalMiddlewares.length - 1;i >= 0; i--) {
            const m = finalMiddlewares[i];
            if (m && typeof m === "object" && typeof m.onResponse === "function") {
              const result = await m.onResponse({
                request,
                response,
                schemaPath,
                params,
                options,
                id
              });
              if (result) {
                if (!(result instanceof Response)) {
                  throw new Error("onResponse: must return new Response() when modifying the response");
                }
                response = result;
              }
            }
          }
        }
      }
      const contentLength = response.headers.get("Content-Length");
      if (response.status === 204 || request.method === "HEAD" || contentLength === "0" && !response.headers.get("Transfer-Encoding")?.includes("chunked")) {
        return response.ok ? { data: undefined, response } : { error: undefined, response };
      }
      if (response.ok) {
        const getResponseData = async () => {
          if (parseAs === "stream") {
            return response.body;
          }
          if (parseAs === "json" && !contentLength) {
            const raw = await response.text();
            return raw ? JSON.parse(raw) : undefined;
          }
          return await response[parseAs]();
        };
        return { data: await getResponseData(), response };
      }
      let error = await response.text();
      try {
        error = JSON.parse(error);
      } catch {}
      return { error, response };
    }
    return {
      request(method, url, init) {
        return coreFetch(url, { ...init, method: method.toUpperCase() });
      },
      GET(url, init) {
        return coreFetch(url, { ...init, method: "GET" });
      },
      PUT(url, init) {
        return coreFetch(url, { ...init, method: "PUT" });
      },
      POST(url, init) {
        return coreFetch(url, { ...init, method: "POST" });
      },
      DELETE(url, init) {
        return coreFetch(url, { ...init, method: "DELETE" });
      },
      OPTIONS(url, init) {
        return coreFetch(url, { ...init, method: "OPTIONS" });
      },
      HEAD(url, init) {
        return coreFetch(url, { ...init, method: "HEAD" });
      },
      PATCH(url, init) {
        return coreFetch(url, { ...init, method: "PATCH" });
      },
      TRACE(url, init) {
        return coreFetch(url, { ...init, method: "TRACE" });
      },
      use(...middleware) {
        for (const m of middleware) {
          if (!m) {
            continue;
          }
          if (typeof m !== "object" || !(("onRequest" in m) || ("onResponse" in m) || ("onError" in m))) {
            throw new Error("Middleware must be an object with one of `onRequest()`, `onResponse() or `onError()`");
          }
          globalMiddlewares.push(m);
        }
      },
      eject(...middleware) {
        for (const m of middleware) {
          const i = globalMiddlewares.indexOf(m);
          if (i !== -1) {
            globalMiddlewares.splice(i, 1);
          }
        }
      }
    };
  }
  function serializePrimitiveParam(name, value, options) {
    if (value === undefined || value === null) {
      return "";
    }
    if (typeof value === "object") {
      throw new Error("Deeply-nested arrays/objects aren’t supported. Provide your own `querySerializer()` to handle these.");
    }
    return `${name}=${options?.allowReserved === true ? value : encodeURIComponent(value)}`;
  }
  function serializeObjectParam(name, value, options) {
    if (!value || typeof value !== "object") {
      return "";
    }
    const values = [];
    const joiner = {
      simple: ",",
      label: ".",
      matrix: ";"
    }[options.style] || "&";
    if (options.style !== "deepObject" && options.explode === false) {
      for (const k in value) {
        values.push(k, options.allowReserved === true ? value[k] : encodeURIComponent(value[k]));
      }
      const final2 = values.join(",");
      switch (options.style) {
        case "form": {
          return `${name}=${final2}`;
        }
        case "label": {
          return `.${final2}`;
        }
        case "matrix": {
          return `;${name}=${final2}`;
        }
        default: {
          return final2;
        }
      }
    }
    for (const k in value) {
      const finalName = options.style === "deepObject" ? `${name}[${k}]` : k;
      values.push(serializePrimitiveParam(finalName, value[k], options));
    }
    const final = values.join(joiner);
    return options.style === "label" || options.style === "matrix" ? `${joiner}${final}` : final;
  }
  function serializeArrayParam(name, value, options) {
    if (!Array.isArray(value)) {
      return "";
    }
    if (options.explode === false) {
      const joiner2 = { form: ",", spaceDelimited: "%20", pipeDelimited: "|" }[options.style] || ",";
      const final = (options.allowReserved === true ? value : value.map((v) => encodeURIComponent(v))).join(joiner2);
      switch (options.style) {
        case "simple": {
          return final;
        }
        case "label": {
          return `.${final}`;
        }
        case "matrix": {
          return `;${name}=${final}`;
        }
        default: {
          return `${name}=${final}`;
        }
      }
    }
    const joiner = { simple: ",", label: ".", matrix: ";" }[options.style] || "&";
    const values = [];
    for (const v of value) {
      if (options.style === "simple" || options.style === "label") {
        values.push(options.allowReserved === true ? v : encodeURIComponent(v));
      } else {
        values.push(serializePrimitiveParam(name, v, options));
      }
    }
    return options.style === "label" || options.style === "matrix" ? `${joiner}${values.join(joiner)}` : values.join(joiner);
  }
  function createQuerySerializer(options) {
    return function querySerializer(queryParams) {
      const search = [];
      if (queryParams && typeof queryParams === "object") {
        for (const name in queryParams) {
          const value = queryParams[name];
          if (value === undefined || value === null) {
            continue;
          }
          if (Array.isArray(value)) {
            if (value.length === 0) {
              continue;
            }
            search.push(serializeArrayParam(name, value, {
              style: "form",
              explode: true,
              ...options?.array,
              allowReserved: options?.allowReserved || false
            }));
            continue;
          }
          if (typeof value === "object") {
            search.push(serializeObjectParam(name, value, {
              style: "deepObject",
              explode: true,
              ...options?.object,
              allowReserved: options?.allowReserved || false
            }));
            continue;
          }
          search.push(serializePrimitiveParam(name, value, options));
        }
      }
      return search.join("&");
    };
  }
  function defaultPathSerializer(pathname, pathParams) {
    let nextURL = pathname;
    for (const match of pathname.match(PATH_PARAM_RE) ?? []) {
      let name = match.substring(1, match.length - 1);
      let explode = false;
      let style = "simple";
      if (name.endsWith("*")) {
        explode = true;
        name = name.substring(0, name.length - 1);
      }
      if (name.startsWith(".")) {
        style = "label";
        name = name.substring(1);
      } else if (name.startsWith(";")) {
        style = "matrix";
        name = name.substring(1);
      }
      if (!pathParams || pathParams[name] === undefined || pathParams[name] === null) {
        continue;
      }
      const value = pathParams[name];
      if (Array.isArray(value)) {
        nextURL = nextURL.replace(match, serializeArrayParam(name, value, { style, explode }));
        continue;
      }
      if (typeof value === "object") {
        nextURL = nextURL.replace(match, serializeObjectParam(name, value, { style, explode }));
        continue;
      }
      if (style === "matrix") {
        nextURL = nextURL.replace(match, `;${serializePrimitiveParam(name, value)}`);
        continue;
      }
      nextURL = nextURL.replace(match, style === "label" ? `.${encodeURIComponent(value)}` : encodeURIComponent(value));
    }
    return nextURL;
  }
  function defaultBodySerializer(body, headers) {
    if (body instanceof FormData) {
      return body;
    }
    if (headers) {
      const contentType = headers.get instanceof Function ? headers.get("Content-Type") ?? headers.get("content-type") : headers["Content-Type"] ?? headers["content-type"];
      if (contentType === "application/x-www-form-urlencoded") {
        return new URLSearchParams(body).toString();
      }
    }
    return JSON.stringify(body);
  }
  function createFinalURL(pathname, options) {
    let finalURL = `${options.baseUrl}${pathname}`;
    if (options.params?.path) {
      finalURL = options.pathSerializer(finalURL, options.params.path);
    }
    let search = options.querySerializer(options.params.query ?? {});
    if (search.startsWith("?")) {
      search = search.substring(1);
    }
    if (search) {
      finalURL += `?${search}`;
    }
    return finalURL;
  }
  function mergeHeaders(...allHeaders) {
    const finalHeaders = new Headers;
    for (const h of allHeaders) {
      if (!h || typeof h !== "object") {
        continue;
      }
      const iterator = h instanceof Headers ? h.entries() : Object.entries(h);
      for (const [k, v] of iterator) {
        if (v === null) {
          finalHeaders.delete(k);
        } else if (Array.isArray(v)) {
          for (const v2 of v) {
            finalHeaders.append(k, v2);
          }
        } else if (v !== undefined) {
          finalHeaders.set(k, v);
        }
      }
    }
    return finalHeaders;
  }
  function removeTrailingSlash(url) {
    if (url.endsWith("/")) {
      return url.substring(0, url.length - 1);
    }
    return url;
  }

  // ../api/src/plugin.ts
  var PLUGIN_API_CLIENT_BASE_URL = "https://plugin.slab.local/";
  var BODYLESS_STATUS_CODES = new Set([204, 205, 304]);
  function createSlabPluginApiFetch(transport, options = {}) {
    return async (input, init) => {
      const bridgeRequest = await toBridgeRequest(input, init, options);
      const bridgeResponse = await transport(bridgeRequest);
      const responseBody = BODYLESS_STATUS_CODES.has(bridgeResponse.status) ? null : bridgeResponse.body;
      return new Response(responseBody, {
        status: bridgeResponse.status,
        headers: bridgeResponse.headers
      });
    };
  }
  function createSlabPluginApiClient(transport, options = {}) {
    return createClient({
      baseUrl: PLUGIN_API_CLIENT_BASE_URL,
      fetch: createSlabPluginApiFetch(transport, options)
    });
  }
  async function toBridgeRequest(input, init, options) {
    const request = createAbsoluteRequest(input, init);
    const url = new URL(request.url);
    if (url.origin !== new URL(PLUGIN_API_CLIENT_BASE_URL).origin) {
      throw new Error("Plugin API clients can only request Slab API paths.");
    }
    const path = `${url.pathname}${url.search}`;
    assertSlabPluginApiSurface(request.method, path);
    return {
      method: request.method,
      path,
      headers: headersToRecord(request.headers),
      body: await readRequestBody(request),
      timeoutMs: options.timeoutMs
    };
  }
  function createAbsoluteRequest(input, init) {
    if (input instanceof Request) {
      return new Request(input, init);
    }
    return new Request(new URL(String(input), PLUGIN_API_CLIENT_BASE_URL), init);
  }
  async function readRequestBody(request) {
    if (request.method === "GET" || request.method === "HEAD") {
      return null;
    }
    const body = await request.clone().text();
    return body.length > 0 ? body : null;
  }
  function headersToRecord(headers) {
    const record = {};
    headers.forEach((value, key) => {
      record[key] = value;
    });
    return record;
  }

  // src/index.ts
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
    const invokeApiRequest = (request) => {
      assertSlabPluginApiSurface(request.method, request.path);
      return requireCore(target).invoke("plugin_api_request", { request });
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
        invoke: (command, args) => requireCore(target).invoke(command, args)
      },
      api: {
        client: apiClient,
        fetch: apiFetch,
        request: invokeApiRequest,
        requestJson: async (request) => {
          const response = await invokeApiRequest(serializeJsonRequest(request));
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
