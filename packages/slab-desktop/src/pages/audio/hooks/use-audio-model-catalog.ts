import { useMemo } from 'react';

import api from '@slab/api';
import { toCatalogModelList, type CatalogModel } from '@slab/api/models';
import { usePersistedHeaderSelect } from '@/hooks/use-persisted-header-select';
import { HEADER_SELECT_KEYS } from '@/layouts/header-controls';

export function useAudioModelCatalog() {
  const {
    data: transcriptionCatalogModels,
    isLoading: transcriptionModelsLoading,
    error: transcriptionModelsError,
    refetch: refetchTranscriptionModels,
  } = api.useQuery('get', '/v1/models', {
    params: {
      query: {
        capability: 'audio_transcription',
      },
    },
  });
  const {
    data: vadCatalogModels,
    isLoading: vadModelsLoading,
    error: vadModelsError,
    refetch: refetchVadModels,
  } = api.useQuery('get', '/v1/models', {
    params: {
      query: {
        capability: 'audio_vad',
      },
    },
  });

  const whisperTranscribeModels = useMemo(
    () => toCatalogModelList(transcriptionCatalogModels).filter((model) => model.kind === 'local'),
    [transcriptionCatalogModels],
  );

  const whisperVadModels = useMemo(
    () => toCatalogModelList(vadCatalogModels).filter((model) => model.kind === 'local'),
    [vadCatalogModels],
  );

  const audioModels = useMemo(() => {
    const merged = new Map<string, CatalogModel>();
    whisperTranscribeModels.forEach((model) => {
      merged.set(model.id, model);
    });
    whisperVadModels.forEach((model) => {
      merged.set(model.id, model);
    });
    return Array.from(merged.values());
  }, [whisperTranscribeModels, whisperVadModels]);

  const catalogModelsLoading = transcriptionModelsLoading || vadModelsLoading;
  const catalogModelsError = transcriptionModelsError ?? vadModelsError;
  const { value: selectedModelId, setValue: setSelectedModelId } = usePersistedHeaderSelect({
    key: HEADER_SELECT_KEYS.audioModel,
    options: whisperTranscribeModels,
    isLoading: catalogModelsLoading,
  });

  const selectedModel = useMemo(
    () => whisperTranscribeModels.find((model) => model.id === selectedModelId),
    [whisperTranscribeModels, selectedModelId],
  );

  return {
    audioModels,
    catalogModelsError,
    catalogModelsLoading,
    refetchTranscriptionModels,
    refetchVadModels,
    selectedModel,
    selectedModelId,
    setSelectedModelId,
    whisperTranscribeModels,
    whisperVadModels,
  };
}
