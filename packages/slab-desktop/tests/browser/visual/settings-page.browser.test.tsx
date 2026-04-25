import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import SettingsPage from '@/pages/settings';
import type { SettingsDocumentResponse } from '@/pages/settings/types';
import { renderDesktopScene } from '../test-utils';

const { mockUseSettingsAutosave } = vi.hoisted(() => ({
  mockUseSettingsAutosave: vi.fn<() => unknown>(),
}));

vi.mock('@/pages/settings/hooks/use-settings-autosave', () => ({
  useSettingsAutosave: mockUseSettingsAutosave,
}));

vi.mock('@/hooks/use-global-header-meta', () => ({
  usePageHeader: vi.fn<() => void>(),
  usePageHeaderControl: vi.fn<() => void>(),
}));

const { mockApiUseQuery } = vi.hoisted(() => ({
  mockApiUseQuery: vi.fn<() => unknown>(),
}));

vi.mock('@slab/api', () => ({
  apiClient: {
    GET: vi.fn<(...args: unknown[]) => unknown>(),
    POST: vi.fn<(...args: unknown[]) => unknown>(),
    PUT: vi.fn<(...args: unknown[]) => unknown>(),
    DELETE: vi.fn<(...args: unknown[]) => unknown>(),
  },
  default: {
    useQuery: mockApiUseQuery,
    useMutation: vi.fn<() => unknown>(() => ({
      isPending: false,
      mutateAsync: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    })),
  },
  getErrorMessage: vi.fn<(err: Error) => string>((err) => err.message),
  isApiError: vi.fn<() => boolean>(() => false),
  queryClient: {},
}));

vi.mock('@slab/i18n', () => ({
  useTranslation: vi.fn<() => unknown>(() => ({
    t: vi.fn<(key: string) => string>((key) => key),
  })),
}));

const createVoidMock = () => vi.fn<(...args: unknown[]) => void>();
const createAsyncVoidMock = () =>
  vi.fn<(...args: unknown[]) => Promise<void>>().mockResolvedValue(undefined);

function createMockSettingsData(overrides: Partial<SettingsDocumentResponse> = {}): SettingsDocumentResponse {
  return {
    schema_version: 2,
    settings_path: '/etc/slab/settings.json',
    warnings: [],
    sections: [
      {
        id: 'general',
        title: 'General',
        description_md: 'Desktop application preferences shared across the frontend shell.',
        subsections: [
          {
            id: 'general',
            title: 'General',
            description_md: 'Choose how the desktop app should present shared interface preferences.',
            properties: [
              {
                pmid: 'general.language',
                label: 'Interface Language',
                description_md: 'Choose how the desktop frontend selects translation resources.',
                editable: true,
                effective_value: 'auto',
                is_overridden: false,
                search_terms: [],
                schema: {
                  type: 'string',
                  enum: ['auto', 'en-US', 'zh-CN'],
                  default_value: 'auto',
                },
              },
            ],
          },
        ],
      },
      {
        id: 'runtime',
        title: 'Runtime',
        description_md: 'Shared inference runtime topology, transport, and backend-specific overrides.',
        subsections: [
          {
            id: 'llama',
            title: 'Llama',
            description_md: 'Overrides specific to the GGML llama worker.',
            properties: [
              {
                pmid: 'runtime.ggml.backends.llama.context_length',
                label: 'Context Length',
                description_md: 'Override the llama context window length in tokens.',
                editable: true,
                effective_value: 4096,
                is_overridden: true,
                override_value: 4096,
                search_terms: [],
                schema: {
                  type: 'integer',
                  default_value: null,
                  minimum: 0,
                },
              },
            ],
          },
        ],
      },
    ],
    ...overrides,
  };
}

function createSettingsAutosaveViewModel(overrides = {}) {
  return {
    drafts: {},
    fieldErrors: {},
    fieldStatuses: {},
    resettingPmid: null,
    statusSummary: {
      error: 0,
      saving: 0,
      dirty: 0,
      saved: 0,
    },
    setDraftValue: createVoidMock(),
    resetSetting: createVoidMock(),
    ...overrides,
  };
}

describe('SettingsPage browser visual regression', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('captures the settings page loading state', async () => {
    mockApiUseQuery.mockReturnValue({
      data: null,
      error: null,
      isLoading: true,
      refetch: createAsyncVoidMock(),
    });

    mockUseSettingsAutosave.mockReturnValue(createSettingsAutosaveViewModel());

    await renderDesktopScene(<SettingsPage />, { route: '/settings' });

    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('settings-page-loading.png');
  });

  it('captures the settings page with data', async () => {
    const mockData = createMockSettingsData();
    mockApiUseQuery.mockReturnValue({
      data: mockData,
      error: null,
      isLoading: false,
      refetch: createAsyncVoidMock(),
    });

    mockUseSettingsAutosave.mockReturnValue(createSettingsAutosaveViewModel());

    await renderDesktopScene(<SettingsPage />, { route: '/settings' });

    await expect.element(page.getByRole('heading', { name: 'General' })).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('settings-page-with-data.png');
  });

  it('captures the settings page with pending changes', async () => {
    const mockData = createMockSettingsData();
    mockApiUseQuery.mockReturnValue({
      data: mockData,
      error: null,
      isLoading: false,
      refetch: createAsyncVoidMock(),
    });

    mockUseSettingsAutosave.mockReturnValue(
      createSettingsAutosaveViewModel({
        drafts: {
          'general.language': 'zh-CN',
        },
        statusSummary: {
          error: 0,
          saving: 0,
          dirty: 1,
          saved: 0,
        },
      }),
    );

    await renderDesktopScene(<SettingsPage />, { route: '/settings' });

    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot(
      'settings-page-pending.png',
    );
  });

  it('captures the settings page error state', async () => {
    mockApiUseQuery.mockReturnValue({
      data: null,
      error: new Error('Failed to load settings'),
      isLoading: false,
      refetch: createAsyncVoidMock(),
    });

    mockUseSettingsAutosave.mockReturnValue(createSettingsAutosaveViewModel());

    await renderDesktopScene(<SettingsPage />, { route: '/settings' });

    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('settings-page-error.png');
  });
});
