import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import SettingsPage from '@/pages/settings';
import type { SettingsDocumentResponse } from '@/pages/settings/types';
import { renderDesktopScene } from '../test-utils';

const { mockUseSettingsAutosave } = vi.hoisted(() => ({
  mockUseSettingsAutosave: vi.fn<() => void>(),
}));

vi.mock('@/pages/settings/hooks/use-settings-autosave', () => ({
  useSettingsAutosave: mockUseSettingsAutosave,
}));

vi.mock('@/hooks/use-global-header-meta', () => ({
  usePageHeader: vi.fn<() => void>(),
  usePageHeaderControl: vi.fn<() => void>(),
}));

const { mockApiUseQuery } = vi.hoisted(() => ({
  mockApiUseQuery: vi.fn<() => void>(),
}));

vi.mock('@/lib/api', () => ({
  apiClient: {
    GET: vi.fn<() => void>(),
    POST: vi.fn<() => void>(),
    PUT: vi.fn<() => void>(),
    DELETE: vi.fn<() => void>(),
  },
  default: {
    useQuery: mockApiUseQuery,
    useMutation: vi.fn<() => void>(() => ({
      isPending: false,
      mutateAsync: vi.fn<() => void>().mockResolvedValue(undefined),
    })),
  },
  getErrorMessage: vi.fn<() => void>((err: Error) => err.message),
  isApiError: vi.fn<() => void>(() => false),
  queryClient: {},
}));

vi.mock('@slab/i18n', () => ({
  useTranslation: vi.fn<() => void>(() => ({
    t: vi.fn<() => void>((key: string) => key),
  })),
}));

function createMockSettingsData(overrides: Partial<SettingsDocumentResponse> = {}): SettingsDocumentResponse {
  return {
    schema_version: '1.0.0',
    settings_path: '/etc/slab/settings.toml',
    warnings: [],
    sections: [
      {
        id: 'general',
        title: 'General Settings',
        description_md: 'Basic application configuration',
        subsections: [
          {
            id: 'appearance',
            title: 'Appearance',
            description_md: 'Customize the look and feel',
            properties: [
              {
                pmid: 'general.appearance.theme',
                label: 'Theme',
                description: 'Application color theme',
                schema: {
                  type: 'string',
                  enum: ['light', 'dark', 'auto'],
                  default: 'dark',
                },
                value: 'dark',
                source: 'default',
              },
              {
                pmid: 'general.appearance.language',
                label: 'Language',
                description: 'Interface language',
                schema: {
                  type: 'string',
                  default: 'en',
                },
                value: 'en',
                source: 'default',
              },
            ],
          },
        ],
      },
      {
        id: 'runtime',
        title: 'Runtime Configuration',
        description_md: 'Model runtime settings',
        subsections: [
          {
            id: 'memory',
            title: 'Memory Management',
            description_md: 'Configure memory allocation',
            properties: [
              {
                pmid: 'runtime.memory.max_gb',
                label: 'Max Memory (GB)',
                description: 'Maximum memory allocation',
                schema: {
                  type: 'number',
                  default: 8,
                  minimum: 2,
                  maximum: 64,
                },
                value: 16,
                source: 'user',
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
    setDraftValue: vi.fn<() => void>(),
    resetSetting: vi.fn<() => void>(),
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
      refetch: vi.fn<() => void>().mockResolvedValue(undefined),
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
      refetch: vi.fn<() => void>().mockResolvedValue(undefined),
    });

    mockUseSettingsAutosave.mockReturnValue(createSettingsAutosaveViewModel());

    await renderDesktopScene(<SettingsPage />, { route: '/settings' });

    await expect.element(page.getByRole('heading', { name: 'General Settings' })).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('settings-page-with-data.png');
  });

  it('captures the settings page with pending changes', async () => {
    const mockData = createMockSettingsData();
    mockApiUseQuery.mockReturnValue({
      data: mockData,
      error: null,
      isLoading: false,
      refetch: vi.fn<() => void>().mockResolvedValue(undefined),
    });

    mockUseSettingsAutosave.mockReturnValue(
      createSettingsAutosaveViewModel({
        drafts: {
          'general.appearance.theme': 'light',
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
      refetch: vi.fn<() => void>().mockResolvedValue(undefined),
    });

    mockUseSettingsAutosave.mockReturnValue(createSettingsAutosaveViewModel());

    await renderDesktopScene(<SettingsPage />, { route: '/settings' });

    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('settings-page-error.png');
  });
});
