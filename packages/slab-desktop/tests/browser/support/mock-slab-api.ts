import { vi } from 'vitest';

type ApiClientMethod = (...args: unknown[]) => unknown;

type SlabApiMockOptions = {
  apiClient?: Partial<Record<'DELETE' | 'GET' | 'POST' | 'PUT', ApiClientMethod>>;
  defaultExport?: Partial<Record<'useMutation' | 'useQuery', ApiClientMethod>>;
  extra?: Record<string, unknown>;
  getErrorMessage?: (error: unknown) => string;
  isApiError?: (error: unknown) => boolean;
  queryClient?: unknown;
};

export function createSlabApiMock({
  apiClient,
  defaultExport,
  extra,
  getErrorMessage = (error) => (error instanceof Error ? error.message : String(error)),
  isApiError = () => false,
  queryClient = {},
}: SlabApiMockOptions = {}) {
  return {
    apiClient: {
      DELETE: vi.fn<ApiClientMethod>(),
      GET: vi.fn<ApiClientMethod>(),
      POST: vi.fn<ApiClientMethod>(),
      PUT: vi.fn<ApiClientMethod>(),
      ...apiClient,
    },
    default: {
      useMutation: vi.fn<ApiClientMethod>(() => ({
        isPending: false,
        mutateAsync: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
      })),
      useQuery: vi.fn<ApiClientMethod>(() => ({
        data: null,
        isLoading: false,
        refetch: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
      })),
      ...defaultExport,
    },
    getErrorMessage,
    isApiError: vi.fn<(error: unknown) => boolean>(isApiError),
    queryClient,
    ...extra,
  };
}
