import { useCallback, useEffect, useMemo, useState } from 'react';
import { useInterval } from '@mantine/hooks';
import { useQueryClient } from '@tanstack/react-query';
import { clamp, countBy } from 'lodash-es';
import { toast } from 'sonner';
import { translateServerField, useTranslation } from '@slab/i18n';

import api, { getErrorMessage, getLocalizedErrorMessage, postFormData } from '@slab/api';
import type { components } from '@slab/api/v1';
import {
  modelSupportsCapability,
  toCatalogModelList,
  type CatalogModelRuntimeState,
  type CatalogModelStatus,
  type ModelCapability,
} from '@slab/api/models';
import { isFailedTaskStatus } from '@/pages/task/utils';
import {
  extractTaskId,
  getModelDownloadTask,
  MODEL_DOWNLOAD_POLL_INTERVAL_MS,
  MODEL_DOWNLOAD_TIMEOUT_MS,
  normalizeTaskProgress,
  sleep,
  startModelDownloadTask,
  type DownloadTrackingState,
  type ModelDownloadProgress,
} from '../lib/model-download';
import { useHubModelDownloadStore } from '../store/useHubModelDownloadStore';

const DEFAULT_VISIBLE_COUNT = 10;
export const CATEGORY_OPTIONS = [
  'all',
  'language',
  'vision',
  'audio',
  'coding',
  'embedding',
] as const;
export const STATUS_OPTIONS = ['all', 'ready', 'downloading', 'not_downloaded', 'error'] as const;

export type ModelCategory = (typeof CATEGORY_OPTIONS)[number];
export type ModelFilterStatus = (typeof STATUS_OPTIONS)[number];
export type ModelStatus = CatalogModelStatus;
export type ModelVramRisk = 'unknown' | 'ok' | 'high';
export type ModelItem = {
  id: string;
  display_name: string;
  kind: 'local' | 'cloud';
  category: ModelCategory;
  repo_id: string;
  filename: string;
  capabilities: ModelCapability[];
  backend_ids: string[];
  is_vad_model: boolean;
  status: ModelStatus;
  local_path: string | null;
  pending: boolean;
  runtime_state: CatalogModelRuntimeState | null;
  download_task_id: string | null;
  download_progress: ModelDownloadProgress | null;
  size_bytes: number | null;
  vram_risk: ModelVramRisk;
  updated_at: string;
};

type ImportedModelResponse = components['schemas']['UnifiedModelResponse'];

async function importModelPack(file: File, invalidFileMessage: string): Promise<ImportedModelResponse> {
  if (!isModelPackFile(file)) {
    throw new Error(invalidFileMessage);
  }

  return postFormData('/v1/models/import-pack', file);
}

export function useHubModelCatalog() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [category, setCategory] = useState<ModelCategory>('all');
  const [status, setStatus] = useState<ModelFilterStatus>('all');
  const [visibleCount, setVisibleCount] = useState(DEFAULT_VISIBLE_COUNT);
  const [isCreateOpen, setIsCreateOpen] = useState(false);
  const [createFile, setCreateFile] = useState<File | null>(null);
  const [createModelPending, setCreateModelPending] = useState(false);
  const [modelToDelete, setModelToDelete] = useState<ModelItem | null>(null);
  const [modelToEnhance, setModelToEnhance] = useState<ModelItem | null>(null);
  const [modelActionPendingId, setModelActionPendingId] = useState<string | null>(null);
  const [modelActionErrors, setModelActionErrors] = useState<Record<string, string>>({});
  const downloadTracking = useHubModelDownloadStore((state) => state.downloadTracking);
  const setModelDownloadTracking = useHubModelDownloadStore((state) => state.setDownloadTracking);

  const {
    data,
    error,
    isLoading,
    isRefetching,
    refetch,
  } = api.useQuery('get', '/v1/models');
  const { data: gpuStatus } = api.useQuery('get', '/v1/system/gpu');
  const deleteModelMutation = api.useMutation('delete', '/v1/models/{id}', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const loadModelMutation = api.useMutation('post', '/v1/models/load', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const unloadModelMutation = api.useMutation('post', '/v1/models/unload', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const switchModelMutation = api.useMutation('post', '/v1/models/switch', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });

  const maxFreeGpuMemoryBytes = useMemo(() => {
    const devices = gpuStatus?.devices ?? [];
    if (devices.length === 0) {
      return null;
    }

    return Math.max(
      ...devices.map((device) =>
        Math.max(0, device.total_memory_bytes - device.used_memory_bytes),
      ),
    );
  }, [gpuStatus]);

  const models = useMemo<ModelItem[]>(
    () =>
      toCatalogModelList(data).map((model) =>
        toModelItem(model, downloadTracking[model.id], maxFreeGpuMemoryBytes),
      ),
    [data, downloadTracking, maxFreeGpuMemoryBytes],
  );
  const filteredModels = useMemo(
    () =>
      models.filter((model) => {
        if (category !== 'all' && model.category !== category) {
          return false;
        }

        if (status !== 'all' && model.status !== status) {
          return false;
        }

        return true;
      }),
    [category, models, status],
  );
  const modelCounts = useMemo(() => countBy(models, (model) => {
    if (model.pending) return 'pending';
    if (model.local_path) return 'downloaded';
    return 'other';
  }), [models]);
  const downloadedCount = modelCounts.downloaded ?? 0;
  const pendingCount = modelCounts.pending ?? 0;
  const visibleModels = useMemo(
    () => filteredModels.slice(0, visibleCount),
    [filteredModels, visibleCount],
  );
  const hasMore = visibleModels.length < filteredModels.length;
  const canCreate = Boolean(createFile && !createModelPending);
  const modelActionPending =
    loadModelMutation.isPending ||
    unloadModelMutation.isPending ||
    switchModelMutation.isPending;

  useEffect(() => {
    setVisibleCount(DEFAULT_VISIBLE_COUNT);
  }, [category, status]);

  const hasPendingModels = models.some((model) => model.pending);
  const { start: startCatalogPoll, stop: stopCatalogPoll } = useInterval(() => {
    void refetch();
  }, 3000);

  useEffect(() => {
    if (hasPendingModels) {
      startCatalogPoll();
      return stopCatalogPoll;
    }

    stopCatalogPoll();
    return undefined;
  }, [hasPendingModels, startCatalogPoll, stopCatalogPoll]);

  function resetCreateState() {
    setCreateFile(null);
  }

  function setCreateOpen(open: boolean) {
    setIsCreateOpen(open);
    if (!open && !createModelPending) {
      resetCreateState();
    }
  }

  function updateCreateFile(file: File | null) {
    setCreateFile(file);
  }

  const loadMore = useCallback(() => {
    setVisibleCount((current) =>
      current >= filteredModels.length
        ? current
        : clamp(current + DEFAULT_VISIBLE_COUNT, 0, filteredModels.length),
    );
  }, [filteredModels.length]);

  async function createModel() {
    if (!createFile || createModelPending) {
      return;
    }

    setCreateModelPending(true);
    try {
      const created = await importModelPack(createFile, t('pages.hub.error.onlySlabPacks'));

      toast.success(t('pages.hub.toast.imported'), {
        description:
          typeof created?.display_name === 'string' && created.display_name.trim()
            ? created.display_name
            : createFile.name,
      });

      setCategory('all');
      setStatus('all');
      setCreateOpen(false);
      void refetch();
    } catch (createError) {
      toast.error(t('pages.hub.toast.importFailed'), {
        description: getErrorMessage(createError),
      });
    } finally {
      setCreateModelPending(false);
    }
  }

  const waitForTaskToFinish = async (modelId: string, taskId: string) => {
    const deadline = Date.now() + MODEL_DOWNLOAD_TIMEOUT_MS;

    while (Date.now() < deadline) {
      // eslint-disable-next-line no-await-in-loop
      const task = await getModelDownloadTask(taskId);
      setModelDownloadTracking(modelId, {
        taskId,
        progress: normalizeTaskProgress(task.progress, t),
      });

      if (task.status === 'succeeded') {
        return;
      }

      if (isFailedTaskStatus(task.status)) {
        throw new Error(
          translateServerField(task.i18n, 'error_msg', task.error_msg, t) ||
            t('pages.hub.error.taskEndedWithStatus', {
              taskId,
              status: task.status,
            }),
        );
      }

      // eslint-disable-next-line no-await-in-loop
      await sleep(MODEL_DOWNLOAD_POLL_INTERVAL_MS);
    }

    throw new Error(t('pages.hub.error.downloadTimedOut'));
  };

  const refreshCatalogAndFindModel = async (modelId: string) => {
    const refreshed = await refetch();
    const refreshedModels = toCatalogModelList(refreshed.data);
    return refreshedModels.find((model) => model.id === modelId);
  };

  const refreshRuntimeState = useCallback(async () => {
    await Promise.all([
      refetch(),
      queryClient.invalidateQueries({
        predicate: (query) => {
          const key = JSON.stringify(query.queryKey);
          return key.includes('/v1/models') || key.includes('/v1/backends/status');
        },
      }),
    ]);
  }, [queryClient, refetch]);

  async function trackModelDownload(model: ModelItem, taskId: string) {
    try {
      await waitForTaskToFinish(model.id, taskId);

      const refreshedModel = await refreshCatalogAndFindModel(model.id);
      if (!refreshedModel?.local_path) {
        throw new Error(t('pages.hub.error.missingDownloadedPath'));
      }

      toast.success(t('pages.hub.toast.downloaded'), {
        description: model.display_name,
      });
    } catch (downloadError) {
      toast.error(t('pages.hub.toast.downloadFailed'), {
        description: getLocalizedErrorMessage(downloadError, t),
      });
    } finally {
      setModelDownloadTracking(model.id, null);
      void refetch();
    }
  }

  async function downloadModel(model: ModelItem) {
    if (!canDownloadModel(model)) {
      return;
    }

    try {
      const response = await startModelDownloadTask(model.id);
      const taskId = extractTaskId(response);

      if (!taskId) {
        throw new Error(t('pages.hub.error.startDownloadFailed'));
      }

      setModelDownloadTracking(model.id, {
        taskId,
        progress: null,
      });
      toast.success(t('pages.hub.toast.downloadStarted'), {
        description: model.display_name,
      });
      void refetch();
      void trackModelDownload(model, taskId);
    } catch (downloadError) {
      toast.error(t('pages.hub.toast.downloadFailed'), {
        description: getLocalizedErrorMessage(downloadError, t),
      });
    }
  }

  async function deleteModel() {
    if (!modelToDelete) {
      return;
    }

    try {
      await deleteModelMutation.mutateAsync({
        params: { path: { id: modelToDelete.id } },
      });

      toast.success(t('pages.hub.toast.removed'), {
        description: modelToDelete.display_name,
      });
      setModelToDelete(null);
      void refreshRuntimeState();
    } catch (deleteError) {
      toast.error(t('pages.hub.toast.deleteFailed'), {
        description: getLocalizedErrorMessage(deleteError, t),
      });
    }
  }

  const runModelAction = useCallback(
    async (
      model: ModelItem,
      successTitle: string,
      action: () => Promise<unknown>,
    ) => {
      if (!canRunModelLifecycleAction(model) || modelActionPending) {
        return;
      }

      setModelActionPendingId(model.id);
      setModelActionErrors((current) => {
        const next = { ...current };
        delete next[model.id];
        return next;
      });
      try {
        await action();
        toast.success(successTitle, {
          description: model.display_name,
        });
        await refreshRuntimeState();
      } catch (actionError) {
        const message = getLocalizedErrorMessage(actionError, t);
        setModelActionErrors((current) => ({
          ...current,
          [model.id]: message,
        }));
        toast.error(t('pages.hub.toast.actionFailed'), {
          description: message,
        });
      } finally {
        setModelActionPendingId(null);
      }
    },
    [modelActionPending, refreshRuntimeState, t],
  );

  const loadModel = useCallback(
    (model: ModelItem) =>
      runModelAction(model, t('pages.hub.toast.loaded'), () =>
        loadModelMutation.mutateAsync({
          body: {
            model_id: model.id,
          },
        }),
      ),
    [loadModelMutation, runModelAction, t],
  );

  const unloadModel = useCallback(
    (model: ModelItem) =>
      runModelAction(model, t('pages.hub.toast.unloaded'), () =>
        unloadModelMutation.mutateAsync({
          body: {
            model_id: model.id,
          },
        }),
      ),
    [runModelAction, t, unloadModelMutation],
  );

  const switchModel = useCallback(
    (model: ModelItem) =>
      runModelAction(model, t('pages.hub.toast.switched'), () =>
        switchModelMutation.mutateAsync({
          body: {
            model_id: model.id,
          },
        }),
      ),
    [runModelAction, switchModelMutation, t],
  );

  return {
    category,
    setCategory,
    status,
    setStatus,
    isCreateOpen,
    setCreateOpen,
    createFileName: createFile?.name ?? null,
    setCreateFile: updateCreateFile,
    modelToDelete,
    setModelToDelete,
    modelToEnhance,
    setModelToEnhance,
    models,
    filteredModels,
    visibleModels,
    hasMore,
    loadMore,
    downloadedCount,
    pendingCount,
    isLoading,
    isRefetching,
    error,
    dataErrorMessage: error ? getLocalizedErrorMessage(error, t) : null,
    refetch,
    canCreate,
    createModel,
    downloadModel,
    deleteModel,
    loadModel,
    unloadModel,
    switchModel,
    createModelPending,
    deleteModelPending: deleteModelMutation.isPending,
    modelActionPendingId,
    modelActionErrors,
    modelActionPending,
  };
}

export function canDownloadModel(
  model: Pick<ModelItem, 'kind' | 'local_path' | 'pending' | 'repo_id' | 'filename'>,
) {
  return (
    model.kind === 'local' &&
    !model.local_path &&
    !model.pending &&
    model.repo_id.trim().length > 0 &&
    model.filename.trim().length > 0
  );
}

export function canRunModelLifecycleAction(
  model: Pick<ModelItem, 'kind' | 'local_path' | 'pending'>,
) {
  return model.kind === 'local' && Boolean(model.local_path) && !model.pending;
}

export function getModelUseRoute(model: Pick<ModelItem, 'category'>): string | null {
  switch (model.category) {
    case 'language':
    case 'coding':
      return '/';
    case 'vision':
      return '/image';
    case 'audio':
      return '/audio';
    case 'embedding':
    case 'all':
      return null;
  }
}

export function classifyByCapabilities(
  model: Pick<ModelItem, 'display_name' | 'kind' | 'backend_ids' | 'repo_id' | 'filename' | 'capabilities'>,
): ModelCategory {
  const haystack = `${model.display_name} ${model.kind} ${model.backend_ids.join(' ')} ${model.repo_id} ${model.filename}`
    .toLowerCase()
    .trim();

  if (model.capabilities.includes('image_embedding')) {
    return 'embedding';
  }

  if (
    model.capabilities.includes('image_generation') ||
    model.capabilities.includes('video_generation')
  ) {
    return 'vision';
  }

  if (
    model.capabilities.includes('audio_transcription') ||
    model.capabilities.includes('audio_vad')
  ) {
    return 'audio';
  }

  if (
    model.capabilities.includes('chat_generation') ||
    model.capabilities.includes('text_generation')
  ) {
    return 'language';
  }

  if (haystack.includes('embed')) {
    return 'embedding';
  }

  if (
    haystack.includes('stable diffusion') ||
    haystack.includes('sdxl') ||
    haystack.includes('vision') ||
    haystack.includes('image')
  ) {
    return 'vision';
  }

  if (
    haystack.includes('whisper') ||
    haystack.includes('audio') ||
    haystack.includes('speech') ||
    haystack.includes('transcrib')
  ) {
    return 'audio';
  }

  if (
    haystack.includes('coder') ||
    haystack.includes('codegen') ||
    haystack.includes('programming')
  ) {
    return 'coding';
  }

  if (
    model.capabilities.includes('chat_generation') ||
    model.capabilities.includes('text_generation')
  ) {
    return 'language';
  }

  return 'language';
}

function isModelPackFile(file: File): boolean {
  return file.name.trim().toLowerCase().endsWith('.slab');
}

export function toModelItem(
  model: ReturnType<typeof toCatalogModelList>[number],
  tracking: DownloadTrackingState | undefined,
  maxFreeGpuMemoryBytes: number | null,
): ModelItem {
  const status = model.status;
  const pending = status === 'downloading';

  return {
    id: model.id,
    display_name: model.display_name,
    kind: model.kind,
    category: classifyByCapabilities(model),
    repo_id: model.repo_id,
    filename: model.filename,
    capabilities: model.capabilities,
    backend_ids: model.backend_ids,
    is_vad_model: modelSupportsCapability(model, 'audio_vad'),
    status,
    local_path: model.local_path,
    pending,
    runtime_state: model.runtime_state,
    download_task_id: tracking?.taskId ?? null,
    download_progress: tracking?.progress ?? null,
    size_bytes: model.size_bytes,
    vram_risk: getVramRisk(model.size_bytes, maxFreeGpuMemoryBytes),
    updated_at: model.updated_at,
  };
}

function getVramRisk(sizeBytes: number | null, maxFreeGpuMemoryBytes: number | null): ModelVramRisk {
  if (!sizeBytes || !maxFreeGpuMemoryBytes) {
    return 'unknown';
  }

  return sizeBytes * 1.2 > maxFreeGpuMemoryBytes ? 'high' : 'ok';
}
