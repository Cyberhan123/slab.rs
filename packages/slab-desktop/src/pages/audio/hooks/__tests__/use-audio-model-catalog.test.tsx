import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from 'vitest-browser-react';

const { useAiModelMock } = vi.hoisted(() => ({
  useAiModelMock: vi.fn<(options?: { capability?: string }) => unknown>(),
}));

vi.mock('@/hooks/use-ai-model', () => ({ useAiModel: useAiModelMock }));

import type {
  AiModel,
  EnsureDownloadedResult,
  EnsureLoadedResult,
  UseAiModelResult,
} from '@/hooks/use-ai-model';
import { useAudioModelCatalog } from '../use-audio-model-catalog';

function aiModel(overrides: Partial<AiModel> = {}): AiModel {
  return {
    backend_id: null,
    backend_ids: [],
    capabilities: ['audio_transcription'],
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
      .fn<(modelId: string) => Promise<EnsureDownloadedResult>>()
      .mockResolvedValue({ model: aiModel(), modelPath: null, downloadedNow: false }),
    load: vi.fn<(modelId: string) => Promise<unknown>>().mockResolvedValue({}),
    switchTo: vi.fn<(modelId: string) => Promise<unknown>>().mockResolvedValue({}),
    unload: vi.fn<(modelId: string) => Promise<unknown>>().mockResolvedValue({}),
    ensureLoaded: vi
      .fn<(modelId: string, options?: { forceDownload?: boolean }) => Promise<EnsureLoadedResult>>()
      .mockResolvedValue({
        model: aiModel(),
        modelPath: null,
        downloadedNow: false,
        loadedNow: false,
        runtimeStatus: null,
      }),
    ...overrides,
  };
}

function installCatalogs(transcription: UseAiModelResult, vad: UseAiModelResult) {
  useAiModelMock.mockImplementation((options) =>
    options?.capability === 'audio_vad' ? vad : transcription,
  );
}

describe('useAudioModelCatalog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('merges transcription and vad local models into a single catalog', async () => {
    installCatalogs(
      catalogResult({ localModels: [aiModel({ id: 'whisper-1' })] }),
      catalogResult({ localModels: [aiModel({ id: 'vad-1' })] }),
    );

    const { result } = await renderHook(() => useAudioModelCatalog());

    expect(result.current.audioModels.map((model) => model.id)).toEqual(['whisper-1', 'vad-1']);
    expect(result.current.whisperTranscribeModels.map((model) => model.id)).toEqual(['whisper-1']);
    expect(result.current.whisperVadModels.map((model) => model.id)).toEqual(['vad-1']);
  });

  it('aggregates loading and error state across both catalogs', async () => {
    installCatalogs(
      catalogResult({ loading: true, error: new Error('transcription down') }),
      catalogResult({ loading: false, error: null }),
    );

    const { result } = await renderHook(() => useAudioModelCatalog());

    expect(result.current.catalogModelsLoading).toBe(true);
    expect(result.current.catalogModelsError).toBeInstanceOf(Error);
  });

  it('routes ensure-downloaded to the vad catalog for vad models', async () => {
    const transcriptionEnsure = vi
      .fn<(modelId: string) => Promise<EnsureDownloadedResult>>()
      .mockResolvedValue({ model: aiModel(), modelPath: null, downloadedNow: false });
    const vadEnsure = vi
      .fn<(modelId: string) => Promise<EnsureDownloadedResult>>()
      .mockResolvedValue({ model: aiModel(), modelPath: null, downloadedNow: false });
    installCatalogs(
      catalogResult({ localModels: [aiModel({ id: 'whisper-1' })], ensureDownloaded: transcriptionEnsure }),
      catalogResult({ localModels: [aiModel({ id: 'vad-1' })], ensureDownloaded: vadEnsure }),
    );

    const { result, act } = await renderHook(() => useAudioModelCatalog());

    await act(async () => {
      await result.current.ensureDownloadedAudioModel('vad-1');
    });
    await act(async () => {
      await result.current.ensureDownloadedAudioModel('whisper-1');
    });

    expect(vadEnsure).toHaveBeenCalledWith('vad-1');
    expect(transcriptionEnsure).toHaveBeenCalledWith('whisper-1');
  });

  it('forwards the transcription selection and lifecycle surface', async () => {
    const transcriptionSetSelectedId = vi.fn<(value: string) => void>();
    installCatalogs(
      catalogResult({
        selectedId: 'whisper-1',
        setSelectedId: transcriptionSetSelectedId,
        status: { ...catalogResult().status, busy: true },
      }),
      catalogResult(),
    );

    const { result } = await renderHook(() => useAudioModelCatalog());

    expect(result.current.selectedModelId).toBe('whisper-1');
    expect(result.current.setSelectedModelId).toBe(transcriptionSetSelectedId);
    expect(result.current.modelLifecycleBusy).toBe(true);
  });
});
