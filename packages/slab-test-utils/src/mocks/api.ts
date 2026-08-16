import { vi } from 'vitest';

/** A mocked openapi-fetch verb (loose signature; resolved values set per test). */
export type ApiVerb = (...args: unknown[]) => Promise<unknown>;

/**
 * Mock shape for `@slab/api`. Covers the union observed across slab-desktop
 * tests: the `default` react-query wrapper (`useQuery` / `useMutation`), the
 * `apiClient` openapi-fetch instance (all five HTTP verbs), and the named
 * helpers `getErrorMessage` / `getLocalizedErrorMessage` / `postFormData`.
 *
 * No single test mocks both `default` and `apiClient` — they serve disjoint
 * scenarios (react-query hooks vs. direct client calls). The factory provides
 * the full union; each test reads only the handles it needs via re-import +
 * `vi.mocked()`.
 */
export interface ApiMockShape {
  apiClient: Record<'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE', ApiVerb>;
  default: {
    useQuery: (...args: unknown[]) => unknown;
    useMutation: (...args: unknown[]) => unknown;
  };
  getErrorMessage: (error: unknown) => string;
  getLocalizedErrorMessage: (error: unknown) => string;
  postFormData: (path: string, file: File) => Promise<unknown>;
}

const identityErrorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

const newVerb = (): ApiVerb => vi.fn<ApiVerb>();

/**
 * Build a fresh `@slab/api` mock shape. Each call creates new `vi.fn()` handles
 * so tests stay isolated.
 */
export function setupApiMock(overrides: Partial<ApiMockShape> = {}): ApiMockShape {
  return {
    apiClient: {
      GET: newVerb(),
      POST: newVerb(),
      PUT: newVerb(),
      PATCH: newVerb(),
      DELETE: newVerb(),
    },
    default: {
      useQuery: vi.fn<(...args: unknown[]) => unknown>(),
      useMutation: vi.fn<(...args: unknown[]) => unknown>(),
    },
    getErrorMessage: identityErrorMessage,
    getLocalizedErrorMessage: identityErrorMessage,
    postFormData: vi.fn<(path: string, file: File) => Promise<unknown>>(),
    ...overrides,
  };
}
