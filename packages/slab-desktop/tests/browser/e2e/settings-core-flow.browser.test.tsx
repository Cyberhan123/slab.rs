import { page } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import SettingsPage from '@/pages/settings';
import type { SettingsDocumentResponse } from '@/pages/settings/types';
import { renderDesktopScene } from '../test-utils';

const {
  mockMutateAsync,
  mockRefetch,
  mockSettingsData,
} = vi.hoisted(() => ({
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

vi.mock('@/hooks/use-global-header-meta', () => ({
  usePageHeader: vi.fn<() => void>(),
}));

vi.mock('@slab/api', () => ({
  default: {
    useMutation: vi.fn<() => unknown>(() => ({
      isPending: false,
      mutateAsync: mockMutateAsync,
    })),
    useQuery: vi.fn<() => unknown>(() => ({
      data: mockSettingsData,
      error: null,
      isLoading: false,
      refetch: mockRefetch,
    })),
  },
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : String(error)),
  isApiError: vi.fn<() => boolean>(() => false),
}));

describe('settings core flow e2e', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('auto-saves edited settings and resets an override through the API contract', async () => {
    await renderDesktopScene(<SettingsPage />, { route: '/settings' });

    const contextLength = page.getByPlaceholder('Enter a whole number');
    await contextLength.fill('8192');
    await vi.advanceTimersByTimeAsync(700);

    await vi.waitFor(() => {
      expect(mockMutateAsync).toHaveBeenCalledWith({
        body: {
          op: 'set',
          value: 8192,
        },
        params: {
          path: {
            pmid: 'runtime.ggml.backends.llama.context_length',
          },
        },
      });
    });

    mockMutateAsync.mockClear();
    await page.getByRole('button', { name: 'Reset' }).click();

    await vi.waitFor(() => {
      expect(mockMutateAsync).toHaveBeenCalledWith({
        body: {
          op: 'unset',
        },
        params: {
          path: {
            pmid: 'runtime.ggml.backends.llama.context_length',
          },
        },
      });
    });
  });
});
