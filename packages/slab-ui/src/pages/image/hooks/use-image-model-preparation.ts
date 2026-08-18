import { useMemo } from 'react';
import { toast } from 'sonner';
import { useTranslation } from '@slab/i18n';

import { useAiModel } from '@slab/ui/hooks/use-ai-model';
import { HEADER_SELECT_KEYS } from '@slab/ui/layouts/header';

import { toImageModelOption, type ImageModelOption } from '../lib/image-model-option';

export type { ImageModelOption };

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
    () => diffusionModels.map(toImageModelOption),
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
      toast.success(t('common.toasts.modelDownloaded', { model: selectedModel.display_name }));
    }

    if (!modelPath) {
      throw new Error(t('common.errors.missingDownloadedPath'));
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
