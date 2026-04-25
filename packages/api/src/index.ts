/**
 * Shared Slab HTTP API entry point.
 *
 * The generated OpenAPI contract lives in `v1.d.ts`; regenerate it with
 * `bun run api` from the repo root while slab-server is running.
 */

import createFetchClient from "openapi-fetch";
import createClient from "openapi-react-query";

import { SERVER_BASE_URL, normalizeApiBaseUrl } from "./config";
import { errorMiddleware } from "./errors";
import type { paths } from "./v1.d.ts";

export type SlabApiClientOptions = {
  baseUrl?: string | null;
  fetch?: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
  useErrorMiddleware?: boolean;
};

function buildClientConfig(options: SlabApiClientOptions = {}) {
  return {
    baseUrl: `${normalizeApiBaseUrl(options.baseUrl ?? SERVER_BASE_URL)}/`,
    fetch: (options.fetch ?? fetch) as typeof fetch,
  } as const;
}

export function createSlabApiFetchClient(options: SlabApiClientOptions = {}) {
  const client = createFetchClient<paths>(buildClientConfig(options));

  if (options.useErrorMiddleware) {
    client.use(errorMiddleware);
  }

  return client;
}

export function createSlabApiQueryHooks(options: SlabApiClientOptions = {}) {
  return createClient(
    createSlabApiFetchClient({
      ...options,
      useErrorMiddleware: options.useErrorMiddleware ?? true,
    }),
  );
}

export const apiClient = createSlabApiFetchClient();

const api = createSlabApiQueryHooks();

export default api;
export type { components, operations, paths } from "./v1.d.ts";
export type { ApiErrorResponse } from "./errors";
export {
  ApiError,
  ErrorCodes,
  NetworkError,
  TimeoutError,
  errorMiddleware,
  getErrorCode,
  getErrorMessage,
  isApiError,
} from "./errors";
