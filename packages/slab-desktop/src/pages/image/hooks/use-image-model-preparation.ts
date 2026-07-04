import { useMemo } from 'react';
import { toast } from 'sonner';
import { useTranslation } from '@slab/i18n';

import { useAiModel } from '@/hooks/use-ai-model';
import { HEADER_SELECT_KEYS } from '@/layouts/header';

export type ImageModelOption = {
  id: string;
  label: string;
  downloaded: boolean;
  pending: boolean;
  local_path: string | null;
};

export function useImageModelPreparation() {
  const { t } = useTranslation();
  const imageModels = useAiModel({
    capability: 'image_generation',
    storageKey: HEADER_SELECT_KEYS.imageModel,
    localOnly: true,
    getDefaultModelId: (models) => models.find((model) => Boolean(model.local_path))?.id,
  });

  const diffusionModels = imageModels.localModels;
  const modelOptions = useMemo<ImageModelOption[]>(
    () =>
      diffusionModels.map((model) => ({
        id: model.id,
        label: model.display_name,
        downloaded: Boolean(model.local_path),
        pending: model.pending,
        local_path: model.local_path ?? null,
      })),
    [diffusionModels],
  );

  const prepareSelectedModel = async (): Promise<string> => {
    if (!imageModels.selectedId) {
      throw new Error(t('pages.image.error.selectModelFirst'));
    }

    const selectedModel = diffusionModels.find((item) => item.id === imageModels.selectedId);
    if (!selectedModel) {
      throw new Error(t('pages.image.error.selectedModelUnavailable'));
    }

    const { modelPath, downloadedNow } = await imageModels.ensureLoaded(imageModels.selectedId);
    if (downloadedNow) {
      toast.success(t('pages.image.toast.downloaded', { model: selectedModel.display_name }));
    }

    if (!modelPath) {
      throw new Error(t('pages.image.error.missingDownloadedPath'));
    }

    return modelPath;
  };

  return {
    catalogLoading: imageModels.loading,
    isPreparingModel: imageModels.status.busy,
    modelOptions,
    prepareSelectedModel,
    selectedModelId: imageModels.selectedId,
    setSelectedModelId: imageModels.setSelectedId,
  };
}
