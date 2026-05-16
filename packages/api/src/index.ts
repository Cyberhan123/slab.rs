/**
 * Shared Slab HTTP API entry point.
 *
 * The generated OpenAPI contract lives in `v1.d.ts`; regenerate it with
 * `bun run gen:api` from the repo root. The generator will start `slab-server`
 * automatically when `/api-docs/openapi.json` is not already available.
 */

import createFetchClient from "openapi-fetch";
import createClient from "openapi-react-query";

import { SERVER_BASE_URL, normalizeApiBaseUrl } from "./config";
import { ApiError, errorMiddleware } from "./errors";
import type { components, paths } from "./v1.d.ts";

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

type FormDataUploadPath = "/v1/models/import-pack" | "/v1/plugins/import-pack";
type ImportModelPackResponse = components["schemas"]["UnifiedModelResponse"];
type ImportPluginPackResponse = components["schemas"]["PluginResponse"];
type PostFormDataOptions = Pick<SlabApiClientOptions, "baseUrl" | "fetch">;

export function postFormData(
  path: "/v1/models/import-pack",
  file: File,
  options?: PostFormDataOptions,
): Promise<ImportModelPackResponse>;
export function postFormData(
  path: "/v1/plugins/import-pack",
  file: File,
  options?: PostFormDataOptions,
): Promise<ImportPluginPackResponse>;
export async function postFormData(
  path: FormDataUploadPath,
  file: File,
  options: PostFormDataOptions = {},
): Promise<ImportModelPackResponse | ImportPluginPackResponse> {
  const body = new FormData();
  body.set("file", file);

  const requestFetch = (options.fetch ?? fetch) as typeof fetch;
  const response = await requestFetch(`${normalizeApiBaseUrl(options.baseUrl ?? SERVER_BASE_URL)}${path}`, {
    body,
    method: "POST",
  });

  if (!response.ok) {
    let errorData: unknown;
    try {
      errorData = await response.clone().json();
    } catch {
      try {
        errorData = await response.clone().text();
      } catch {
        errorData = undefined;
      }
    }
    throw ApiError.fromResponse(response, errorData);
  }

  const data = await response.json();
  if (!data) {
    throw new Error(`Request to ${path} returned an empty response.`);
  }

  return data;
}

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
