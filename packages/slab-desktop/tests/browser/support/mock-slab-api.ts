import { vi } from 'vitest';

type ApiClientMethod = (...args: unknown[]) => unknown;

type SlabApiMockOptions = {
  apiClient?: Partial<Record<'DELETE' | 'GET' | 'POST' | 'PUT', ApiClientMethod>>;
  defaultExport?: Partial<Record<'useMutation' | 'useQuery', ApiClientMethod>>;
  extra?: Record<string, unknown>;
  getErrorData?: (error: unknown) => unknown | undefined;
  getLocalizedErrorMessage?: (error: unknown) => string;
  getErrorMessage?: (error: unknown) => string;
  isApiError?: (error: unknown) => boolean;
  isApiErrorResponse?: (error: unknown) => boolean;
  isRetryable?: (error: unknown) => boolean;
  queryClient?: unknown;
};

class MockApiError extends Error {
  code: number;
  data?: unknown;
  status?: number;

  constructor(code: number, message: string, data?: unknown, status?: number) {
    super(message);
    this.name = 'ApiError';
    this.code = code;
    this.data = data;
    this.status = status;
  }

  static fromResponse(response: Response, errorData?: unknown) {
    return new MockApiError(response.status * 10, response.statusText, errorData, response.status);
  }
}

export function createSlabApiMock({
  apiClient,
  defaultExport,
  extra,
  getErrorData = () => undefined,
  getLocalizedErrorMessage = (error) => (error instanceof Error ? error.message : String(error)),
  getErrorMessage = (error) => (error instanceof Error ? error.message : String(error)),
  isApiError = () => false,
  isApiErrorResponse = () => false,
  isRetryable = () => false,
  queryClient = {},
}: SlabApiMockOptions = {}) {
  const fetchClient = {
    DELETE: vi.fn<ApiClientMethod>(),
    GET: vi.fn<ApiClientMethod>(),
    POST: vi.fn<ApiClientMethod>(),
    PUT: vi.fn<ApiClientMethod>(),
    ...apiClient,
  };
  const queryHooks = {
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
  };

  return {
    ApiError: MockApiError,
    ErrorCodes: {},
    NetworkError: MockApiError,
    TimeoutError: MockApiError,
    apiClient: fetchClient,
    createSlabApiFetchClient: vi.fn<() => unknown>(() => fetchClient),
    createSlabApiQueryHooks: vi.fn<() => unknown>(() => queryHooks),
    default: queryHooks,
    getErrorData: vi.fn<(error: unknown) => unknown | undefined>(getErrorData),
    getLocalizedErrorMessage: vi.fn<(error: unknown) => string>(getLocalizedErrorMessage),
    getErrorMessage,
    isApiError: vi.fn<(error: unknown) => boolean>(isApiError),
    isApiErrorResponse: vi.fn<(error: unknown) => boolean>(isApiErrorResponse),
    isRetryable: vi.fn<(error: unknown) => boolean>(isRetryable),
    postFormData: vi.fn<() => Promise<unknown>>().mockResolvedValue({}),
    queryClient,
    ...extra,
  };
}
