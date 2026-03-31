import { useEffect, useMemo, useState } from 'react';
import { toast } from 'sonner';

import api from '@/lib/api';
import { toCatalogModelList } from '@/lib/api/models';

const DIFFUSION_BACKEND_ID = 'ggml.diffusion';
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
  const [selectedModelId, setSelectedModelId] = useState('');
  const [loadedModelId, setLoadedModelId] = useState<string | null>(null);

  const {
    data: catalogModels,
    isLoading: catalogLoading,
    refetch: refetchCatalogModels,
  } = api.useQuery('get', '/v1/models');
  const downloadModelMutation = api.useMutation('post', '/v1/models/download');
  const loadModelMutation = api.useMutation('post', '/v1/models/load');
  const switchModelMutation = api.useMutation('post', '/v1/models/switch');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');

  const normalizedCatalogModels = useMemo(
    () => toCatalogModelList(catalogModels),
    [catalogModels],
  );

  const diffusionModels = useMemo(
    () =>
      normalizedCatalogModels.filter((model) => model.backend_id === DIFFUSION_BACKEND_ID),
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

  useEffect(() => {
    if (modelOptions.length === 0) {
      setSelectedModelId('');
      return;
    }

    const exists = modelOptions.some((model) => model.id === selectedModelId);
    if (!selectedModelId || !exists) {
      const downloaded = modelOptions.find((model) => model.downloaded);
      setSelectedModelId(downloaded?.id ?? modelOptions[0].id);
    }
  }, [modelOptions, selectedModelId]);

  const waitForTaskToFinish = async (taskId: string) => {
    const deadline = Date.now() + MODEL_DOWNLOAD_TIMEOUT_MS;

    while (Date.now() < deadline) {
      const task = (await getTaskMutation.mutateAsync({
        params: { path: { id: taskId } },
      })) as { status: string; error_msg?: string | null };

      if (task.status === 'succeeded') {
        return;
      }

      if (task.status === 'failed' || task.status === 'cancelled' || task.status === 'interrupted') {
        throw new Error(task.error_msg ?? `Task ${taskId} ended with status: ${task.status}`);
      }

      await sleep(MODEL_DOWNLOAD_POLL_INTERVAL_MS);
    }

    throw new Error('Model download timed out');
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
      throw new Error('Selected model does not exist in catalog');
    }

    if (model.backend_id !== DIFFUSION_BACKEND_ID) {
      throw new Error(`Selected model does not support ${DIFFUSION_BACKEND_ID}`);
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
      throw new Error('Failed to start model download task');
    }

    await waitForTaskToFinish(taskId);

    const refreshedModel = await refreshCatalogAndFindModel(modelId);
    if (!refreshedModel?.local_path) {
      throw new Error('Model download completed, but local_path is empty');
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
      throw new Error('Please select an image model first.');
    }

    const selectedModel = diffusionModels.find((item) => item.id === selectedModelId);
    if (!selectedModel) {
      throw new Error('Selected model is not available');
    }

    const { modelPath, downloadedNow } = await ensureDownloadedModelPath(selectedModelId);
    if (downloadedNow) {
      toast.success(`Downloaded ${selectedModel.display_name}`);
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

      toast.message('Model load failed, re-downloading and retrying once...');

      const retry = await ensureDownloadedModelPath(selectedModelId, true);
      if (retry.downloadedNow) {
        toast.success(`Downloaded ${selectedModel.display_name}`);
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
