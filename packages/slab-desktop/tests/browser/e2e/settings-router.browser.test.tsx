import { createMemoryRouter } from 'react-router-dom';
import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { createDesktopRoutes } from '@/router';
import type { SettingsDocumentResponse } from '@/pages/settings/types';
import { renderDesktopScene } from '../test-utils';

const {
  mockApiUseQuery,
  mockMutateAsync,
  mockRefetch,
  mockSettingsData,
} = vi.hoisted(() => ({
  mockApiUseQuery: vi.fn<(...args: unknown[]) => unknown>(),
  mockMutateAsync: vi.fn<() => Promise<Record<string, never>>>().mockResolvedValue({}),
  mockRefetch: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  mockSettingsData: {
    schema_version: 2,
    settings_path: 'C:/Slab/settings.json',
    warnings: [],
    sections: [
      {
        description_md: 'Runtime preferences used by the desktop shell.',
        id: 'runtime',
        subsections: [
          {
            description_md: 'Overrides specific to the GGML llama worker.',
            id: 'llama',
            properties: [
              {
                description_md: 'Override the llama context window length in tokens.',
                editable: true,
                effective_value: 4096,
                is_overridden: true,
                label: 'Context Length',
                override_value: 4096,
                pmid: 'runtime.ggml.backends.llama.context_length',
                schema: {
                  default_value: null,
                  minimum: 0,
                  type: 'integer',
                },
                search_terms: [],
              },
            ],
            title: 'Llama',
          },
        ],
        title: 'Runtime',
      },
    ],
  } as SettingsDocumentResponse,
}));

vi.mock('@slab/api', async () => {
  const { createSlabApiMock } = await import('../support/mock-slab-api');

  return createSlabApiMock({
    defaultExport: {
      useMutation: vi.fn<() => unknown>(() => ({
        isPending: false,
        mutateAsync: mockMutateAsync,
      })),
      useQuery: mockApiUseQuery,
    },
  });
});

vi.mock('@/lib/workspace-bridge', () => ({
  WORKSPACE_STATE_QUERY_KEY: ['workspace-state'],
  workspaceState: () => Promise.resolve({ config: null, current: null, recent: [] }),
}));

describe('settings router integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockApiUseQuery.mockImplementation((method, path) => {
      if (method !== 'get') {
        return {
          data: null,
          error: null,
          isLoading: false,
          refetch: mockRefetch,
        };
      }

      switch (path) {
        case '/v1/setup/status':
          return {
            data: { initialized: true },
            error: null,
            isLoading: false,
            refetch: mockRefetch,
          };
        case '/v1/settings':
          return {
            data: mockSettingsData,
            error: null,
            isLoading: false,
            refetch: mockRefetch,
          };
        case '/v1/settings/{pmid}':
          return {
            data: { effective_value: 'en-US' },
            error: null,
            isLoading: false,
            refetch: mockRefetch,
          };
        case '/v1/plugins':
          return {
            data: [],
            error: null,
            isLoading: false,
            refetch: mockRefetch,
          };
        default:
          return {
            data: null,
            error: null,
            isLoading: false,
            refetch: mockRefetch,
          };
      }
    });
  });

  it('opens settings through the production data-router route without tripping useBlocker', async () => {
    const router = createMemoryRouter(createDesktopRoutes(), {
      initialEntries: ['/settings'],
    });

    await renderDesktopScene(null, { router });

    await expect.element(page.getByRole('heading', { name: 'Runtime' })).toBeVisible();
    expect(document.querySelector('header.shell-topbar')).not.toBeNull();
    await expect.element(page.getByTestId('sidebar-link-settings')).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(document.body.textContent).not.toContain('useBlocker must be used within a data router');
    expect(document.body.textContent).not.toContain('Page crashed');
  });
});
