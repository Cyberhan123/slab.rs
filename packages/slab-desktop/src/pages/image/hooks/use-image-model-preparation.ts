import { useMemo, useState } from 'react';
import { toast } from 'sonner';
import { useTranslation } from '@slab/i18n';

import { usePersistedHeaderSelect } from '@/hooks/use-persisted-header-select';
import api from '@slab/api';
import { toCatalogModelList } from '@slab/api/models';
import { HEADER_SELECT_KEYS } from '@/layouts/header-controls';

const MODEL_DOWNLOAD_POLL_INTERVAL_MS = 2_000;
const MODEL_DOWNLOAD_TIMEOUT_MS = 30 * 60 * 1_000;

export type ImageModelOption = {
  id: string;
  label: string;
  downloaded: boolean;
  pending: boolean;
  local_path: string | null;
};

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

const extractTaskId = (payload: unknown): string | null => {
  if (typeof payload !== 'object' || payload === null) return null;
  const taskId =
    (payload as { operation_id?: unknown }).operation_id ??
    (payload as { task_id?: unknown }).task_id;
  if (typeof taskId !== 'string') return null;
  const trimmed = taskId.trim();
  return trimmed.length > 0 ? trimmed : null;
};

export function useImageModelPreparation() {
  const { t } = useTranslation();
  const [loadedModelId, setLoadedModelId] = useState<string | null>(null);

  const {
    data: catalogModels,
    isLoading: catalogLoading,
    refetch: refetchCatalogModels,
  } = api.useQuery('get', '/v1/models', {
    params: {
      query: {
        capability: 'image_generation',
      },
    },
  });
  const downloadModelMutation = api.useMutation('post', '/v1/models/download');
  const loadModelMutation = api.useMutation('post', '/v1/models/load');
  const switchModelMutation = api.useMutation('post', '/v1/models/switch');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');

  const normalizedCatalogModels = useMemo(
    () => toCatalogModelList(catalogModels),
    [catalogModels],
  );

  const diffusionModels = useMemo(
    () => normalizedCatalogModels.filter((model) => model.kind === 'local'),
    [normalizedCatalogModels],
  );

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
  const { value: selectedModelId, setValue: setSelectedModelId } = usePersistedHeaderSelect({
    key: HEADER_SELECT_KEYS.imageModel,
    options: modelOptions,
    isLoading: catalogLoading,
    getDefaultValue: (options) => options.find((option) => option.downloaded)?.id,
  });

  const waitForTaskToFinish = async (taskId: string) => {
    const deadline = Date.now() + MODEL_DOWNLOAD_TIMEOUT_MS;

    while (Date.now() < deadline) {
      // eslint-disable-next-line no-await-in-loop
      const task = (await getTaskMutation.mutateAsync({
        params: { path: { id: taskId } },
      })) as { status: string; error_msg?: string | null };

      if (task.status === 'succeeded') {
        return;
      }

      if (task.status === 'failed' || task.status === 'cancelled' || task.status === 'interrupted') {
        throw new Error(
          task.error_msg ??
            t('pages.hub.error.taskEndedWithStatus', {
              taskId,
              status: task.status,
            }),
        );
      }

      // eslint-disable-next-line no-await-in-loop
      await sleep(MODEL_DOWNLOAD_POLL_INTERVAL_MS);
    }

    throw new Error(t('pages.image.error.downloadTimedOut'));
  };

  const refreshCatalogAndFindModel = async (modelId: string) => {
    const refreshed = await refetchCatalogModels();
    const models = toCatalogModelList(refreshed.data);
    return models.find((model) => model.id === modelId);
  };

  const ensureDownloadedModelPath = async (
    modelId: string,
    forceDownload = false,
  ): Promise<{ modelPath: string; downloadedNow: boolean }> => {
    let model = diffusionModels.find((item) => item.id === modelId);
    if (!model) {
      model = await refreshCatalogAndFindModel(modelId);
    }

    if (!model) {
        throw new Error(t('pages.image.error.selectedModelMissing'));
    }

    if (model.kind !== 'local') {
      throw new Error(t('pages.image.error.selectedModelNotLocal'));
    }

    if (model.local_path && !forceDownload) {
      return { modelPath: model.local_path, downloadedNow: false };
    }

    const downloadResponse = await downloadModelMutation.mutateAsync({
      body: {
        model_id: modelId,
      },
    });
    const taskId = extractTaskId(downloadResponse);

    if (!taskId) {
      throw new Error(t('pages.image.error.startDownloadFailed'));
    }

    await waitForTaskToFinish(taskId);

    const refreshedModel = await refreshCatalogAndFindModel(modelId);
    if (!refreshedModel?.local_path) {
      throw new Error(t('pages.image.error.missingDownloadedPath'));
    }

    return { modelPath: refreshedModel.local_path, downloadedNow: true };
  };

  const loadOrSwitchSelectedModel = async (modelId: string) => {
    const shouldSwitch = Boolean(loadedModelId && loadedModelId !== selectedModelId);
    if (shouldSwitch) {
      await switchModelMutation.mutateAsync({
        body: {
          model_id: modelId,
        },
      });
      return;
    }

    await loadModelMutation.mutateAsync({
      body: {
        model_id: modelId,
      },
    });
  };

  const prepareSelectedModel = async (): Promise<string> => {
    if (!selectedModelId) {
      throw new Error(t('pages.image.error.selectModelFirst'));
    }

    const selectedModel = diffusionModels.find((item) => item.id === selectedModelId);
    if (!selectedModel) {
      throw new Error(t('pages.image.error.selectedModelUnavailable'));
    }

    const { modelPath, downloadedNow } = await ensureDownloadedModelPath(selectedModelId);
    if (downloadedNow) {
      toast.success(t('pages.image.toast.downloaded', { model: selectedModel.display_name }));
    }

    if (loadedModelId === selectedModelId) {
      return modelPath;
    }

    try {
      await loadOrSwitchSelectedModel(selectedModelId);
    } catch (firstLoadError) {
      if (downloadedNow) {
        throw firstLoadError;
      }

      toast.message(t('pages.image.toast.modelLoadRetry'));

      const retry = await ensureDownloadedModelPath(selectedModelId, true);
      if (retry.downloadedNow) {
        toast.success(t('pages.image.toast.downloaded', { model: selectedModel.display_name }));
      }

      await loadOrSwitchSelectedModel(selectedModelId);
      setLoadedModelId(selectedModelId);
      return retry.modelPath;
    }

    setLoadedModelId(selectedModelId);
    return modelPath;
  };

  return {
    catalogLoading,
    isPreparingModel:
      loadModelMutation.isPending ||
      switchModelMutation.isPending ||
      downloadModelMutation.isPending,
    modelOptions,
    prepareSelectedModel,
    selectedModelId,
    setSelectedModelId,
  };
}
