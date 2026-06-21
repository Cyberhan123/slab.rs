import { MutationCache, QueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import i18n from "@slab/i18n";
import { getLocalizedErrorMessage, isRetryable } from "@slab/api";

declare module "@tanstack/react-query" {
  interface Register {
    mutationMeta: {
      skipGlobalErrorToast?: boolean;
    };
  }
}

export function isAbortLikeError(error: unknown) {
  return (
    (error instanceof DOMException && error.name === "AbortError") ||
    (error instanceof Error && error.name === "AbortError")
  );
}

export function shouldRetryQuery(failureCount: number, error: unknown) {
  return failureCount < 2 && isRetryable(error);
}

export function shouldShowGlobalMutationErrorToast(
  error: unknown,
  meta: { skipGlobalErrorToast?: boolean } | undefined,
) {
  return !meta?.skipGlobalErrorToast && !isAbortLikeError(error);
}

export function toErrorToastId(error: unknown) {
  if (error instanceof Error && error.message.trim()) {
    return `mutation-error:${error.name}:${error.message}`;
  }

  return `mutation-error:${String(error)}`;
}

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      retry: shouldRetryQuery,
      retryDelay: (attempt) => Math.min(10_000, 1_000 * 2 ** attempt) + Math.random() * 300,
      staleTime: 10_000,
    },
    mutations: {
      // Mutations represent user actions and may not be idempotent. Let the
      // caller decide whether a retry button or explicit repeat action is safe.
      retry: false,
    },
  },
  mutationCache: new MutationCache({
    onError: (error, _variables, _context, mutation) => {
      if (!shouldShowGlobalMutationErrorToast(error, mutation.meta)) {
        return;
      }

      toast.error(getLocalizedErrorMessage(error, i18n.t.bind(i18n)), {
        id: toErrorToastId(error),
      });
    },
  }),
});
