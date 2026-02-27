/**
 * Error handling utilities for API calls
 *
 * Provides standardized error types and middleware for handling
 * the consistent error response format from the backend.
 */

import type { Middleware } from "openapi-fetch";

/**
 * Standard error response format from the backend
 */
export interface ApiErrorResponse {
  code: number;
  data: unknown;
  message: string;
}

/**
 * Custom error class for API errors
 */
export class ApiError extends Error {
  code: number;
  data: unknown;

  constructor(code: number, message: string, data?: unknown) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.data = data;
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
      case 5003:
        return "Backend service is not ready. Please ensure all backends are properly configured.";
      case 5000:
        return "An error occurred while processing your request.";
      case 5001:
        return "A database error occurred. Please try again later.";
      case 5002:
        return "An internal server error occurred. Please try again later.";
      default:
        return "An unexpected error occurred. Please try again.";
    }
  }
}

/**
 * Error codes for different error types
 */
export const ErrorCodes = {
  NOT_FOUND: 4004,
  BAD_REQUEST: 4000,
  BACKEND_NOT_READY: 5003,
  RUNTIME_ERROR: 5000,
  DATABASE_ERROR: 5001,
  INTERNAL_ERROR: 5002,
} as const;

/**
 * Middleware for handling API errors
 */
export const errorMiddleware: Middleware = {
  async onResponse({ response }) {
    // If response is ok, let it pass through
    if (response.ok) {
      return undefined;
    }

    // Clone the response to avoid consuming the original
    const clonedResponse = response.clone();

    try {
      // Parse error response as JSON
      const errorData = (await clonedResponse.json()) as ApiErrorResponse;

      // Validate error response format
      if (
        typeof errorData.code === "number" &&
        typeof errorData.message === "string"
      ) {
        // Throw proper ApiError with backend error details
        throw new ApiError(
          errorData.code,
          errorData.message,
          errorData.data
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
        `${response.url}: ${response.status} ${response.statusText}`
      );
    }
  },

  async onError({ error }) {
    // Wrap any other fetch errors
    if (error instanceof Error) {
      return new ApiError(5002, error.message);
    }

    return error;
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
