import createFetchClient from "openapi-fetch";

import { assertSlabPluginApiSurface } from "./permissions";
import type { paths } from "./v1.d.ts";

const PLUGIN_API_CLIENT_BASE_URL = "https://plugin.slab.local/";
const BODYLESS_STATUS_CODES = new Set([204, 205, 304]);

export type SlabApiBridgeRequest = {
  method: string;
  path: string;
  headers?: Record<string, string>;
  body?: string | null;
  timeoutMs?: number | null;
};

export type SlabApiBridgeResponse = {
  status: number;
  headers: Record<string, string>;
  body: string;
};

export type SlabApiBridgeTransport = (
  request: SlabApiBridgeRequest,
) => Promise<SlabApiBridgeResponse>;

export type SlabPluginApiFetchOptions = {
  timeoutMs?: number | null;
};

export type SlabApiFetch = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

export function createSlabPluginApiFetch(
  transport: SlabApiBridgeTransport,
  options: SlabPluginApiFetchOptions = {},
): SlabApiFetch {
  return async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const bridgeRequest = await toBridgeRequest(input, init, options);
    const bridgeResponse = await transport(bridgeRequest);
    const responseBody = BODYLESS_STATUS_CODES.has(bridgeResponse.status)
      ? null
      : bridgeResponse.body;

    return new Response(responseBody, {
      status: bridgeResponse.status,
      headers: bridgeResponse.headers,
    });
  };
}

export function createSlabPluginApiClient(
  transport: SlabApiBridgeTransport,
  options: SlabPluginApiFetchOptions = {},
) {
  return createFetchClient<paths>({
    baseUrl: PLUGIN_API_CLIENT_BASE_URL,
    fetch: createSlabPluginApiFetch(transport, options) as typeof fetch,
  });
}

export type SlabPluginApiClient = ReturnType<typeof createSlabPluginApiClient>;

async function toBridgeRequest(
  input: RequestInfo | URL,
  init: RequestInit | undefined,
  options: SlabPluginApiFetchOptions,
): Promise<SlabApiBridgeRequest> {
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
    timeoutMs: options.timeoutMs,
  };
}

function createAbsoluteRequest(input: RequestInfo | URL, init?: RequestInit): Request {
  if (input instanceof Request) {
    return new Request(input, init);
  }

  return new Request(new URL(String(input), PLUGIN_API_CLIENT_BASE_URL), init);
}

async function readRequestBody(request: Request): Promise<string | null> {
  if (request.method === "GET" || request.method === "HEAD") {
    return null;
  }

  const body = await request.clone().text();
  return body.length > 0 ? body : null;
}

function headersToRecord(headers: Headers): Record<string, string> {
  const record: Record<string, string> = {};
  headers.forEach((value, key) => {
    record[key] = value;
  });
  return record;
}
