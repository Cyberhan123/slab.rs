import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from 'vitest-browser-react';

const { useAiModelMock, toastMock } = vi.hoisted(() => ({
  useAiModelMock: vi.fn<() => unknown>(),
  toastMock: { success: vi.fn<(message: string) => void>() },
}));

vi.mock('@slab/ui/hooks/use-ai-model', () => ({ useAiModel: useAiModelMock }));
vi.mock('sonner', () => ({ toast: toastMock }));
vi.mock('@slab/i18n', () => ({ default: { t: (key: string) => key }, useTranslation: () => ({ t: (key: string) => key }) }));

import type { AiModel, EnsureDownloadedResult, EnsureLoadedResult, UseAiModelResult } from '@slab/ui/hooks/use-ai-model';
import { useImageModelPreparation } from '../use-image-model-preparation';

function aiModel(overrides: Partial<AiModel> = {}): AiModel {
  return {
    backend_id: null,
    backend_ids: [],
    capabilities: ['image_generation'],
    chat_capabilities: null,
    created_at: '2026-01-01T00:00:00Z',
    display_name: 'Model',
    filename: 'model.gguf',
    id: 'model-1',
    kind: 'local',
    local_path: null,
    pending: false,
    repo_id: 'owner/model',
    runtime_state: null,
    size_bytes: null,
    spec: {
      filename: 'model.gguf',
      local_path: null,
      provider_id: null,
      remote_model_id: null,
      repo_id: 'owner/model',
    },
    status: 'ready',
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

function catalogResult(overrides: Partial<UseAiModelResult> = {}): UseAiModelResult {
  return {
    models: [],
    localModels: [],
    options: [],
    selectedId: '',
    setSelectedId: vi.fn<(value: string) => void>(),
    selected: undefined,
    loading: false,
    refetching: false,
    error: null,
    refetch: vi.fn<() => Promise<{ data: unknown }>>().mockResolvedValue({ data: {} }),
    status: { downloading: false, loading: false, switching: false, unloading: false, busy: false },
    download: vi.fn<(modelId: string) => Promise<unknown>>().mockResolvedValue({}),
    ensureDownloaded: vi
      .fn<(modelId: string, options?: { forceDownload?: boolean }) => Promise<EnsureDownloadedResult>>()
      .mockResolvedValue({ model: aiModel(), modelPath: null, downloadedNow: false }),
    load: vi.fn<(modelId: string) => Promise<unknown>>().mockResolvedValue({}),
    switchTo: vi.fn<(modelId: string) => Promise<unknown>>().mockResolvedValue({}),
    unload: vi.fn<(modelId: string) => Promise<unknown>>().mockResolvedValue({}),
    ensureLoaded: vi.fn<(modelId: string) => Promise<EnsureLoadedResult>>().mockResolvedValue({
      model: aiModel(),
      modelPath: null,
      downloadedNow: false,
      loadedNow: false,
      runtimeStatus: null,
    }),
    ...overrides,
  };
}

describe('useImageModelPreparation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('maps local diffusion models to picker options', async () => {
    useAiModelMock.mockReturnValue(
      catalogResult({
        localModels: [aiModel({ id: 'sdxl', display_name: 'SDXL', local_path: '/models/sdxl.gguf' })],
      }),
    );

    const { result } = await renderHook(() => useImageModelPreparation());

    expect(result.current.modelOptions).toEqual([
      { id: 'sdxl', label: 'SDXL', downloaded: true, pending: false, local_path: '/models/sdxl.gguf' },
    ]);
  });

  it('throws when preparing a model without a selection', async () => {
    useAiModelMock.mockReturnValue(catalogResult({ selectedId: '' }));
    const { result, act } = await renderHook(() => useImageModelPreparation());

    let thrown: unknown = null;
    await act(async () => {
      try {
        await result.current.prepareSelectedModel();
      } catch (error) {
        thrown = error;
      }
    });

    expect((thrown as Error).message).toBe('pages.image.error.selectModelFirst');
  });

  it('throws when the selected model is no longer in the catalog', async () => {
    useAiModelMock.mockReturnValue(
      catalogResult({ localModels: [aiModel({ id: 'sdxl' })], selectedId: 'missing' }),
    );
    const { result, act } = await renderHook(() => useImageModelPreparation());

    let thrown: unknown = null;
    await act(async () => {
      try {
        await result.current.prepareSelectedModel();
      } catch (error) {
        thrown = error;
      }
    });

    expect((thrown as Error).message).toBe('pages.image.error.selectedModelUnavailable');
  });

  it('toasts when the model is freshly downloaded and returns its path', async () => {
    useAiModelMock.mockReturnValue(
      catalogResult({
        localModels: [aiModel({ id: 'sdxl', display_name: 'SDXL', local_path: '/models/sdxl.gguf' })],
        selectedId: 'sdxl',
        ensureLoaded: vi.fn<(modelId: string) => Promise<EnsureLoadedResult>>().mockResolvedValue({
          model: aiModel({ id: 'sdxl' }),
          modelPath: '/models/sdxl.gguf',
          downloadedNow: true,
          loadedNow: false,
          runtimeStatus: null,
        }),
      }),
    );
    const { result, act } = await renderHook(() => useImageModelPreparation());

    let modelPath = '';
    await act(async () => {
      modelPath = await result.current.prepareSelectedModel();
    });

    expect(modelPath).toBe('/models/sdxl.gguf');
    expect(toastMock.success).toHaveBeenCalledWith('pages.image.toast.downloaded');
  });

  it('throws when the loaded model exposes no local path', async () => {
    useAiModelMock.mockReturnValue(
      catalogResult({
        localModels: [aiModel({ id: 'sdxl', local_path: null })],
        selectedId: 'sdxl',
        ensureLoaded: vi.fn<(modelId: string) => Promise<EnsureLoadedResult>>().mockResolvedValue({
          model: aiModel({ id: 'sdxl' }),
          modelPath: null,
          downloadedNow: false,
          loadedNow: false,
          runtimeStatus: null,
        }),
      }),
    );
    const { result, act } = await renderHook(() => useImageModelPreparation());

    let thrown: unknown = null;
    await act(async () => {
      try {
        await result.current.prepareSelectedModel();
      } catch (error) {
        thrown = error;
      }
    });

    expect((thrown as Error).message).toBe('pages.image.error.missingDownloadedPath');
  });
});
