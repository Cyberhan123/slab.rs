(function () {
  const ops = globalThis.Deno.core.ops;

  if (typeof globalThis.TextDecoder === "undefined") {
    globalThis.TextDecoder = class TextDecoder {
      decode(input = []) {
        return ops.op_slab_decode_utf8(Array.from(input || []));
      }
    };
  }

  if (typeof globalThis.TextEncoder === "undefined") {
    globalThis.TextEncoder = class TextEncoder {
      encode(input = "") {
        return Uint8Array.from(ops.op_slab_encode_utf8(String(input)));
      }
    };
  }

  function normalizeHeaders(headers) {
    if (!headers) {
      return {};
    }
    if (typeof Headers !== "undefined" && headers instanceof Headers) {
      return Object.fromEntries(headers.entries());
    }
    if (Array.isArray(headers)) {
      return Object.fromEntries(headers);
    }
    return { ...headers };
  }

  function responseFromPayload(payload) {
    const headers = payload.headers || {};
    const bodyBytes = Uint8Array.from(payload.bodyBytes || []);
    const bodyText = () => new TextDecoder().decode(bodyBytes);
    return {
      status: payload.status,
      ok: payload.status >= 200 && payload.status < 300,
      headers,
      text: async () => bodyText(),
      json: async () => JSON.parse(bodyText() || "null"),
      arrayBuffer: async () =>
        bodyBytes.buffer.slice(bodyBytes.byteOffset, bodyBytes.byteOffset + bodyBytes.byteLength),
      bytes: async () => Uint8Array.from(bodyBytes),
    };
  }

  globalThis.Slab = {
    pluginId: ops.op_slab_plugin_id(),
    api: {
      request: (request) => ops.op_slab_api_request(request),
    },
    ui: {
      emit: (topic, data = null) => ops.op_slab_ui_emit({ topic, data }),
    },
  };

  globalThis.fetch = async function slabFetch(input, init = {}) {
    const url = typeof input === "string" ? input : input && input.url;
    if (!url) {
      throw new TypeError("fetch input must be a URL string or Request-like object");
    }
    const request = {
      url,
      method: init.method || (typeof input === "object" && input.method) || "GET",
      headers: normalizeHeaders(init.headers || (typeof input === "object" && input.headers)),
      body: init.body == null ? null : String(init.body),
      timeoutMs: init.timeoutMs,
    };
    return responseFromPayload(await ops.op_slab_fetch(request));
  };

  globalThis.Deno = {
    ...(globalThis.Deno || {}),
    readFile: async (path) => Uint8Array.from(await ops.op_slab_read_file(String(path))),
    readTextFile: async (path) => {
      const bytes = await ops.op_slab_read_file(String(path));
      return new TextDecoder().decode(Uint8Array.from(bytes));
    },
    writeFile: async (path, data) => {
      const bytes = data instanceof Uint8Array ? Array.from(data) : Array.from(new Uint8Array(data));
      await ops.op_slab_write_file({ path: String(path), bytes });
    },
    writeTextFile: async (path, data) => {
      await ops.op_slab_write_file({
        path: String(path),
        bytes: Array.from(new TextEncoder().encode(String(data))),
      });
    },
  };
})();
