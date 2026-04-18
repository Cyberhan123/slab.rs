import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import HubPage from '@/pages/hub';
import type { ModelItem } from '@/pages/hub/hooks/use-hub-model-catalog';
import { renderDesktopScene } from '../test-utils';

const { mockUseHubModelCatalog } = vi.hoisted(() => ({
  mockUseHubModelCatalog: vi.fn<() => unknown>(),
}));

vi.mock('@/pages/hub/hooks/use-hub-model-catalog', () => ({
  useHubModelCatalog: mockUseHubModelCatalog,
  CATEGORY_OPTIONS: ['all', 'language', 'vision', 'audio', 'coding', 'embedding'] as const,
  STATUS_OPTIONS: ['all', 'ready', 'downloading', 'not_downloaded', 'error'] as const,
  canDownloadModel: vi.fn(() => true),
}));

vi.mock('@/hooks/use-global-header-meta', () => ({
  usePageHeader: vi.fn<() => void>(),
  usePageHeaderControl: vi.fn<() => void>(),
}));

vi.mock('@slab/i18n', () => ({
  useTranslation: vi.fn(() => ({
    t: vi.fn((key: string) => key),
  })),
}));

function createMockModel(overrides: Partial<ModelItem> = {}): ModelItem {
  return {
    id: 'model-1',
    display_name: 'Llama 3.2 3B',
    kind: 'local',
    repo_id: 'meta-llama/Llama-3.2-3B',
    filename: 'llama-3.2-3b.gguf',
    capabilities: ['chat_generation', 'text_generation'],
    backend_ids: ['llama'],
    is_vad_model: false,
    status: 'ready',
    local_path: '/models/llama-3.2-3b.gguf',
    pending: false,
    download_task_id: null,
    download_progress: null,
    updated_at: '2024-01-15T10:30:00Z',
    ...overrides,
  };
}

function createHubViewModel(overrides = {}) {
  return {
    category: 'all' as const,
    setCategory: vi.fn(),
    status: 'all' as const,
    setStatus: vi.fn(),
    isCreateOpen: false,
    setCreateOpen: vi.fn(),
    createFileName: null,
    setCreateFile: vi.fn(),
    modelToDelete: null,
    setModelToDelete: vi.fn(),
    modelToEnhance: null,
    setModelToEnhance: vi.fn(),
    models: [] as ModelItem[],
    filteredModels: [] as ModelItem[],
    visibleModels: [] as ModelItem[],
    hasMore: false,
    loadMore: vi.fn(),
    downloadedCount: 0,
    pendingCount: 0,
    isLoading: false,
    isRefetching: false,
    error: null,
    refetch: vi.fn().mockResolvedValue(undefined),
    canCreate: false,
    createModel: vi.fn().mockResolvedValue(undefined),
    downloadModel: vi.fn().mockResolvedValue(undefined),
    deleteModel: vi.fn().mockResolvedValue(undefined),
    createModelPending: false,
    deleteModelPending: false,
    ...overrides,
  };
}

describe('HubPage browser visual regression', () => {
  beforeEach(() => {
    mockUseHubModelCatalog.mockReset();
  });

  it('captures the hub page empty state', async () => {
    mockUseHubModelCatalog.mockReturnValue(
      createHubViewModel({
        isLoading: false,
        models: [],
        filteredModels: [],
        visibleModels: [],
      }),
    );

    await renderDesktopScene(<HubPage />, { route: '/hub' });

    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('hub-page-empty.png');
  });

  it('captures the hub page loading state', async () => {
    mockUseHubModelCatalog.mockReturnValue(
      createHubViewModel({
        isLoading: true,
      }),
    );

    await renderDesktopScene(<HubPage />, { route: '/hub' });

    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('hub-page-loading.png');
  });

  it('captures the hub page with models', async () => {
    const mockModels: ModelItem[] = [
      createMockModel({
        id: 'model-1',
        display_name: 'Llama 3.2 3B',
        status: 'ready',
        local_path: '/models/llama-3.2-3b.gguf',
      }),
      createMockModel({
        id: 'model-2',
        display_name: 'Stable Diffusion XL',
        status: 'not_downloaded',
        local_path: null,
        capabilities: ['image_generation'],
      }),
      createMockModel({
        id: 'model-3',
        display_name: 'Whisper Large V3',
        status: 'ready',
        local_path: '/models/whisper-large-v3.gguf',
        capabilities: ['audio_transcription'],
      }),
    ];

    mockUseHubModelCatalog.mockReturnValue(
      createHubViewModel({
        models: mockModels,
        filteredModels: mockModels,
        visibleModels: mockModels,
        downloadedCount: 2,
        pendingCount: 0,
      }),
    );

    await renderDesktopScene(<HubPage />, { route: '/hub' });

    await expect.element(page.getByText('Llama 3.2 3B')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('hub-page-with-models.png');
  });

  it('captures the hub page error state', async () => {
    mockUseHubModelCatalog.mockReturnValue(
      createHubViewModel({
        error: new Error('Failed to load model catalog'),
      }),
    );

    await renderDesktopScene(<HubPage />, { route: '/hub' });

    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('hub-page-error.png');
  });
});
