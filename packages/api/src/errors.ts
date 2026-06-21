/**
 * Error handling utilities for API calls
 *
 * Provides standardized error types and middleware for handling
 * the consistent error response format from the backend.
 */

import type { Middleware } from "openapi-fetch";

const UNAUTHORIZED_CODE = 4010;
const ADMIN_AUTH_MESSAGE =
  "Admin API authorization failed. Configure server.admin.token or provide the matching bearer token.";

export type ServerI18nMessageRef = {
  key: string;
  params?: Record<string, unknown>;
};

export type ServerI18nPayload = Record<string, ServerI18nMessageRef> | null | undefined;

export type AppCoreErrorData =
  | {
      code: "unsupported_chat_parameter";
      error_type?: string;
      param: string;
    }
  | {
      code: "model_download_unavailable";
      error_type?: string;
      model_id: string;
      param?: string;
      reason: string;
      suggestion: string;
    }
  | {
      code: "runtime_failure";
      detail?: unknown;
      error_type?: string;
      runtime_code: string;
    }
  | (Record<string, unknown> & { code: string });

export type ErrorTranslator = (
  key: string,
  options?: Record<string, unknown>,
) => string;

/**
 * Standard error response format from the backend
 */
export interface ApiErrorResponse {
  code: number;
  data?: unknown;
  message: string;
  i18n?: ServerI18nPayload;
  status?: number;
}

/**
 * Custom error class for API errors
 */
export class ApiError extends Error {
  code: number;
  status?: number;
  data: unknown;
  i18n?: ApiErrorResponse["i18n"];

  constructor(
    code: number,
    message: string,
    data?: unknown,
    status?: number,
    i18n?: ApiErrorResponse["i18n"],
  ) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.status = status;
    this.data = data;
    this.i18n = i18n;
  }

  static fromResponse(response: Response, errorData?: unknown): ApiError {
    if (response.status === 401) {
      return new ApiError(UNAUTHORIZED_CODE, ADMIN_AUTH_MESSAGE, errorData, response.status);
    }

    if (isApiErrorResponse(errorData)) {
      const payload = errorData as ApiErrorResponse;
      return new ApiError(payload.code, payload.message, payload.data, response.status, payload.i18n);
    }

    return new ApiError(
      response.status * 10,
      `${response.status} ${response.statusText}`,
      errorData,
      response.status
    );
  }

  /**
   * Check if this is a client error (4xxx)
   */
  isClientError(): boolean {
    return this.code >= 4000 && this.code < 5000;
  }

  /**
   * Check if this is a server error (5xxx)
   */
  isServerError(): boolean {
    return this.code >= 5000;
  }

  /**
   * Get user-friendly error message based on error code
   */
  getUserMessage(): string {
    if (this.status === 401 || this.code === UNAUTHORIZED_CODE) {
      return ADMIN_AUTH_MESSAGE;
    }

    // If message already contains details, return it
    if (this.message && !this.message.includes("error:")) {
      return this.message;
    }

    // Otherwise, provide generic message based on code
    switch (this.code) {
      case 4000:
        return "Invalid request. Please check your input and try again.";
      case 4004:
        return "The requested resource was not found.";
      case 4009:
        return "The request conflicts with the current state. Refresh and try again.";
      case 4029:
        return "Too many requests. Wait a moment and try again.";
      case 5003:
        return "Backend service is not ready. Please ensure all backends are properly configured.";
      case 5000:
        return "An error occurred while processing your request.";
      case 5001:
        return "A database error occurred. Please try again later.";
      case 5002:
        return "An internal server error occurred. Please try again later.";
      case 5010:
        return "This operation is not implemented yet.";
      default:
        return "An unexpected error occurred. Please try again.";
    }
  }
}

/**
 * Error codes for different error types
 */
export const ErrorCodes = {
  UNAUTHORIZED: UNAUTHORIZED_CODE,
  NOT_FOUND: 4004,
  BAD_REQUEST: 4000,
  CONFLICT: 4009,
  TOO_MANY_REQUESTS: 4029,
  BACKEND_NOT_READY: 5003,
  RUNTIME_ERROR: 5000,
  DATABASE_ERROR: 5001,
  INTERNAL_ERROR: 5002,
  NOT_IMPLEMENTED: 5010,
} as const;

export class NetworkError extends ApiError {
  constructor(message = "Network request failed") {
    super(5002, message);
    this.name = "NetworkError";
  }
}

export class TimeoutError extends ApiError {
  constructor(message = "Request timed out") {
    super(5002, message);
    this.name = "TimeoutError";
  }
}

/**
 * Middleware for handling API errors
 */
export const errorMiddleware: Middleware = {
  async onResponse({ response }) {
    // If response is ok, let it pass through
    if (response.ok) {
      return undefined;
    }

    if (response.status === 401) {
      throw new ApiError(UNAUTHORIZED_CODE, ADMIN_AUTH_MESSAGE, null, response.status);
    }

    // Clone the response to avoid consuming the original
    const clonedResponse = response.clone();

    try {
      // Parse error response as JSON
      const errorData = (await clonedResponse.json()) as ApiErrorResponse;

      // Validate error response format
      if (isApiErrorResponse(errorData)) {
        // Throw proper ApiError with backend error details
        throw new ApiError(
          errorData.code,
          errorData.message,
          errorData.data,
          response.status,
          errorData.i18n,
        );
      }

      // If response doesn't match expected format, throw generic error
      throw new Error(
        `${response.url}: ${response.status} ${response.statusText}`
      );
    } catch (error) {
      // If parsing failed or already an ApiError, re-throw
      if (error instanceof ApiError) {
        throw error;
      }

      // If JSON parsing failed, throw generic error
      throw new Error(
        `${response.url}: ${response.status} ${response.statusText}`, { cause: error }
      );
    }
  },

  async onError({ error }) {
    // Wrap any other fetch errors
    if (error instanceof Error) {
      return new ApiError(5002, error.message);
    }

    return new Error(String(error));
  },
};

/**
 * Helper function to extract error message from unknown error type
 */
export function getErrorMessage(error: unknown): string {
  if (error instanceof ApiError) {
    return error.getUserMessage();
  }

  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

/**
 * Helper function to extract error code from unknown error type
 */
export function getErrorCode(error: unknown): number | undefined {
  if (error instanceof ApiError) {
    return error.code;
  }

  return undefined;
}

/**
 * Type guard to check if error is ApiError
 */
export function isApiError(error: unknown): error is ApiError {
  return error instanceof ApiError;
}

export function isApiErrorResponse(value: unknown): value is ApiErrorResponse {
  return (
    typeof value === "object" &&
    value !== null &&
    "code" in value &&
    typeof (value as { code?: unknown }).code === "number" &&
    "message" in value &&
    typeof (value as { message?: unknown }).message === "string"
  );
}

export function getLocalizedErrorMessage(
  error: unknown,
  t?: ErrorTranslator,
): string {
  if (error instanceof ApiError) {
    const translated = translateErrorField(error.i18n, "message", error.message, t);
    return translated || error.getUserMessage();
  }

  if (isApiErrorResponse(error)) {
    return translateErrorField(error.i18n, "message", error.message, t) || error.message;
  }

  return getErrorMessage(error);
}

export function getErrorData<T = AppCoreErrorData>(error: unknown): T | undefined {
  if (error instanceof ApiError) {
    return error.data === undefined || error.data === null ? undefined : (error.data as T);
  }

  if (isApiErrorResponse(error)) {
    return error.data === undefined || error.data === null ? undefined : (error.data as T);
  }

  return undefined;
}

export function isRetryable(error: unknown): boolean {
  if (error instanceof TimeoutError || error instanceof NetworkError) {
    return true;
  }

  const status =
    error instanceof ApiError
      ? error.status
      : isApiErrorResponse(error)
        ? error.status
        : undefined;
  const code =
    error instanceof ApiError
      ? error.code
      : isApiErrorResponse(error)
        ? error.code
        : undefined;

  if (status === 408 || status === 425 || status === 429 || (status !== undefined && status >= 500)) {
    return status !== 501;
  }

  switch (code) {
    case ErrorCodes.TOO_MANY_REQUESTS:
    case ErrorCodes.BACKEND_NOT_READY:
    case ErrorCodes.RUNTIME_ERROR:
    case ErrorCodes.DATABASE_ERROR:
    case ErrorCodes.INTERNAL_ERROR:
      return true;
    case ErrorCodes.NOT_IMPLEMENTED:
    case ErrorCodes.CONFLICT:
    case ErrorCodes.BAD_REQUEST:
    case ErrorCodes.NOT_FOUND:
    case ErrorCodes.UNAUTHORIZED:
      return false;
    default:
      return false;
  }
}

function translateErrorField(
  i18n: ServerI18nPayload,
  field: string,
  fallback: string | undefined,
  t?: ErrorTranslator,
): string {
  const ref = i18n?.[field];
  if (!ref || !t) {
    return fallback ?? "";
  }

  const translated = t(ref.key, {
    ...ref.params,
    defaultValue: "",
  });
  return translated && translated !== ref.key ? translated : (fallback ?? "");
}
