/**
 * Shared Slab HTTP API entry point.
 *
 * The generated OpenAPI contract lives in `v1.d.ts`; regenerate it with
 * `bun run gen:api` from the repo root. The generator will start `slab-server`
 * automatically when `/api-docs/openapi.json` is not already available.
 */

import createFetchClient from "openapi-fetch";
import type { Middleware } from "openapi-fetch";
import createClient from "openapi-react-query";

import { SERVER_BASE_URL, normalizeApiBaseUrl } from "./config";
import { ApiError, NetworkError, errorMiddleware, isApiErrorResponse } from "./errors";
import type { components, paths } from "./v1.d.ts";

export type SlabApiClientOptions = {
  baseUrl?: string | null;
  fetch?: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
  getAdminToken?: () => string | null;
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

  if (options.getAdminToken) {
    client.use(adminTokenMiddleware(options.getAdminToken));
  }

  if (options.useErrorMiddleware) {
    client.use(errorMiddleware);
  }

  return client;
}

function adminTokenMiddleware(getAdminToken: () => string | null): Middleware {
  return {
    onRequest({ request }) {
      const token = getAdminToken()?.trim();
      if (token) {
        request.headers.set("Authorization", `Bearer ${token}`);
      }
      return request;
    },
  };
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
type PostFormDataOptions = Pick<SlabApiClientOptions, "baseUrl" | "fetch"> & {
  onUploadProgress?: (progress: { loaded: number; total: number | null }) => void;
  signal?: AbortSignal;
};

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
  const endpoint = `${normalizeApiBaseUrl(options.baseUrl ?? SERVER_BASE_URL)}${path}`;

  if (options.onUploadProgress && !options.fetch) {
    return uploadFormDataWithProgress(endpoint, body, options) as Promise<
      ImportModelPackResponse | ImportPluginPackResponse
    >;
  }

  const requestFetch = (options.fetch ?? fetch) as typeof fetch;
  const response = await requestFetch(endpoint, {
    body,
    method: "POST",
    signal: options.signal,
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

function uploadFormDataWithProgress(
  endpoint: string,
  body: FormData,
  options: PostFormDataOptions,
) {
  return new Promise<unknown>((resolve, reject) => {
    const request = new XMLHttpRequest();
    const abort = () => {
      request.abort();
      reject(new DOMException("Upload cancelled", "AbortError"));
    };

    if (options.signal?.aborted) {
      abort();
      return;
    }

    options.signal?.addEventListener("abort", abort, { once: true });
    request.upload.addEventListener("progress", (event) => {
      options.onUploadProgress?.({
        loaded: event.loaded,
        total: event.lengthComputable ? event.total : null,
      });
    });
    request.addEventListener("error", () => {
      reject(new NetworkError(`Request to ${endpoint} failed.`));
    });
    request.addEventListener("abort", () => {
      reject(new DOMException("Upload cancelled", "AbortError"));
    });
    request.addEventListener("load", () => {
      options.signal?.removeEventListener("abort", abort);
      let payload: unknown;
      try {
        payload = request.responseText ? JSON.parse(request.responseText) : undefined;
      } catch {
        payload = request.responseText;
      }

      if (request.status < 200 || request.status >= 300) {
        if (isApiErrorResponse(payload)) {
          reject(
            new ApiError(
              payload.code,
              payload.message,
              payload.data,
              request.status,
              payload.i18n,
            ),
          );
          return;
        }

        reject(new ApiError(request.status * 10, request.statusText, payload, request.status));
        return;
      }

      if (!payload) {
        reject(new Error(`Request to ${endpoint} returned an empty response.`));
        return;
      }

      resolve(payload);
    });
    request.open("POST", endpoint);
    request.send(body);
  });
}

export default api;
export type { components, operations, paths } from "./v1.d.ts";
export type {
  ApiErrorResponse,
  AppCoreErrorData,
  ErrorTranslator,
  ServerI18nMessageRef,
  ServerI18nPayload,
} from "./errors";
export {
  ApiError,
  ErrorCodes,
  NetworkError,
  TimeoutError,
  errorMiddleware,
  getErrorData,
  getErrorCode,
  getErrorMessage,
  getLocalizedErrorMessage,
  isApiErrorResponse,
  isApiError,
  isRetryable,
} from "./errors";
