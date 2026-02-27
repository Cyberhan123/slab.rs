/**
 * Enhanced API client with interceptors and error handling
 */

import createFetchClient from 'openapi-fetch';
import createClient from 'openapi-react-query';
import type { paths } from './v1.d.ts';
import { getApiConfig } from './config';
import { ApiError, NetworkError, TimeoutError } from './errors';
import { logDebug, logInfo, logWarn, logError as logDiagError } from './diagnostics';

// Create fetch client with interceptors
const fetchClient = createFetchClient<paths>({
  baseUrl: getApiConfig().baseUrl,
});

// Wrap fetch to add interceptors
const originalFetch = window.fetch;
window.fetch = async (input, init) => {
  const url = typeof input === 'string' ? input : input.url;
  const method = init?.method || 'GET';

  // Log request
  logDebug('request', {
    method,
    url,
    headers: init?.headers,
    body: init?.body ? '(body present)' : undefined,
  });

  // Add timeout
  const timeout = 30000; // 30 seconds
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), timeout);

  try {
    const response = await originalFetch(input, {
      ...init,
      signal: controller.signal,
    });

    clearTimeout(timeoutId);

    // Log response
    logInfo('response', {
      method,
      url,
      status: response.status,
      ok: response.ok,
      headers: Object.fromEntries(response.headers.entries()),
    });

    // Handle non-JSON responses
    if (!response.ok) {
      const contentType = response.headers.get('content-type');
      let errorData;

      if (contentType?.includes('application/json')) {
        try {
          errorData = await response.clone().json();
        } catch {
          // Failed to parse error as JSON
        }
      }

      logDiagError('error', {
        method,
        url,
        status: response.status,
        errorData,
      });

      throw ApiError.fromResponse(response, errorData);
    }

    return response;
  } catch (error) {
    clearTimeout(timeoutId);

    if (error instanceof Error) {
      if (error.name === 'AbortError') {
        const timeoutError = new TimeoutError();
        logDiagError('error', {
          method,
          url,
          error: 'timeout',
          duration: timeout,
        });
        throw timeoutError;
      }

      if (error instanceof TypeError && error.message.includes('fetch')) {
        const networkError = new NetworkError();
        logDiagError('error', {
          method,
          url,
          error: 'network',
          message: error.message,
        });
        throw networkError;
      }

      logDiagError('error', {
        method,
        url,
        error: error.message,
        stack: error.stack,
      });
    }

    throw error;
  }
};

// Create React Query client
const api = createClient(fetchClient);

// Configure React Query defaults
import { QueryClient } from '@tanstack/react-query';

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: (failureCount, error) => {
        // Don't retry on client errors (4xx)
        if (error instanceof ApiError && error.status >= 400 && error.status < 500) {
          return false;
        }
        // Retry up to 3 times on server errors (5xx) or network errors
        return failureCount < 3;
      },
      retryDelay: (attemptIndex) => {
        // Exponential backoff: 1s, 2s, 4s, max 30s
        return Math.min(1000 * 2 ** attemptIndex, 30000);
      },
      staleTime: 5000, // 5 seconds
      gcTime: 300000, // 5 minutes (formerly cacheTime)
    },
    mutations: {
      retry: false, // Don't retry mutations by default
    },
  },
});

export default api;
export * from './errors';
export * from './diagnostics';
export { getApiConfig, getApiBaseUrlAsync } from './config';
