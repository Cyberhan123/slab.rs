import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render } from 'vitest-browser-react';
import { type ReactElement } from 'react';
import { MemoryRouter } from 'react-router-dom';

export interface RenderWithProvidersOptions {
  /** Override the default no-retry QueryClient (queries + mutations). */
  queryClient?: QueryClient;
  /** Wrap the tree in a MemoryRouter. `true` for default routes, or pass initial entries. */
  router?: boolean | string[];
}

export type RenderWithProvidersResult = Awaited<ReturnType<typeof render>> & {
  queryClient: QueryClient;
};

/**
 * Render a React element wrapped in the providers slab-desktop components
 * commonly need: a `QueryClientProvider` (default client disables query +
 * mutation retries, matching `tests/browser/test-utils.tsx`) and an optional
 * `MemoryRouter`.
 *
 * `render` from `vitest-browser-react` is asynchronous and returns a locator
 * surface, so this helper is async too — callers must `await` it.
 *
 * i18n is intentionally NOT wrapped — unit tests mock `@slab/i18n` at the
 * module level, so `useTranslation` is already a stub and needs no provider.
 */
export async function renderWithProviders(
  ui: ReactElement,
  opts: RenderWithProvidersOptions = {},
): Promise<RenderWithProvidersResult> {
  const queryClient =
    opts.queryClient ??
    new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
  let tree: ReactElement = <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>;
  if (opts.router) {
    const initialEntries = Array.isArray(opts.router) ? opts.router : undefined;
    tree = <MemoryRouter initialEntries={initialEntries}>{tree}</MemoryRouter>;
  }
  return { ...(await render(tree)), queryClient };
}
