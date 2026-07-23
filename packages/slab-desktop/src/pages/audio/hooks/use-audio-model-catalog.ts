import { useMemo } from 'react';

import {
  useAiModel,
  type EnsureDownloadedResult,
} from '@/hooks/use-ai-model';
import { HEADER_SELECT_KEYS } from '@/layouts/header';

import { mergeAudioModels } from '../lib/audio-model-catalog';

export function useAudioModelCatalog() {
  const transcriptionCatalog = useAiModel({
    capability: 'audio_transcription',
    storageKey: HEADER_SELECT_KEYS.audioModel,
    localOnly: true,
  });
  const vadCatalog = useAiModel({
    capability: 'audio_vad',
    localOnly: true,
  });

  const whisperTranscribeModels = transcriptionCatalog.localModels;
  const whisperVadModels = vadCatalog.localModels;

  const audioModels = useMemo(
    () => mergeAudioModels(whisperTranscribeModels, whisperVadModels),
    [whisperTranscribeModels, whisperVadModels],
  );

  const ensureDownloadedAudioModel = async (
    modelId: string,
  ): Promise<EnsureDownloadedResult> => {
    if (whisperVadModels.some((model) => model.id === modelId)) {
      return vadCatalog.ensureDownloaded(modelId);
    }

    return transcriptionCatalog.ensureDownloaded(modelId);
  };

  return {
    audioModels,
    catalogModelsError: transcriptionCatalog.error ?? vadCatalog.error,
    catalogModelsLoading: transcriptionCatalog.loading || vadCatalog.loading,
    ensureDownloadedAudioModel,
    ensureDownloadedTranscriptionModel: transcriptionCatalog.ensureDownloaded,
    ensureDownloadedVadModel: vadCatalog.ensureDownloaded,
    loadTranscriptionModel: transcriptionCatalog.load,
    modelLifecycleBusy: transcriptionCatalog.status.busy || vadCatalog.status.busy,
    refetchTranscriptionModels: transcriptionCatalog.refetch,
    refetchVadModels: vadCatalog.refetch,
    selectedModel: transcriptionCatalog.selected,
    selectedModelId: transcriptionCatalog.selectedId,
    setSelectedModelId: transcriptionCatalog.setSelectedId,
    whisperTranscribeModels,
    whisperVadModels,
  };
}
