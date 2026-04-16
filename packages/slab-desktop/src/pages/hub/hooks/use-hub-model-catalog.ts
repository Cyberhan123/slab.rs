import { useEffect, useMemo, useState } from 'react';
import { toast } from 'sonner';
import { useTranslation } from '@slab/i18n';

import api, { getErrorMessage } from '@/lib/api';
import type { components } from '@/lib/api/v1.d.ts';
import {
  modelSupportsCapability,
  toCatalogModelList,
  type CatalogModelStatus,
  type ModelCapability,
} from '@/lib/api/models';
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
export type ModelItem = {
  id: string;
  display_name: string;
  kind: 'local' | 'cloud';
  repo_id: string;
  filename: string;
  capabilities: ModelCapability[];
  backend_ids: string[];
  is_vad_model: boolean;
  status: ModelStatus;
  local_path: string | null;
  pending: boolean;
  download_task_id: string | null;
  download_progress: ModelDownloadProgress | null;
  updated_at: string;
};

type ImportedModelResponse = components['schemas']['UnifiedModelResponse'];

export function useHubModelCatalog() {
  const { t } = useTranslation();
  const [category, setCategory] = useState<ModelCategory>('all');
  const [status, setStatus] = useState<ModelFilterStatus>('all');
  const [visibleCount, setVisibleCount] = useState(DEFAULT_VISIBLE_COUNT);
  const [isCreateOpen, setIsCreateOpen] = useState(false);
  const [createFile, setCreateFile] = useState<File | null>(null);
  const [createModelPending, setCreateModelPending] = useState(false);
  const [modelToDelete, setModelToDelete] = useState<ModelItem | null>(null);
  const [modelToEnhance, setModelToEnhance] = useState<ModelItem | null>(null);
  const downloadTracking = useHubModelDownloadStore((state) => state.downloadTracking);
  const setModelDownloadTracking = useHubModelDownloadStore((state) => state.setDownloadTracking);

  const {
    data,
    error,
    isLoading,
    isRefetching,
    refetch,
  } = api.useQuery('get', '/v1/models');
  const importModelPackMutation = api.useMutation('post', '/v1/models/import-pack') as unknown as {
    isPending: boolean;
    mutateAsync: (options: { body: FormData }) => Promise<ImportedModelResponse>;
  };
  const deleteModelMutation = api.useMutation('delete', '/v1/models/{id}');

  const models = useMemo<ModelItem[]>(
    () =>
      toCatalogModelList(data).map((model) =>
        toModelItem(model, downloadTracking[model.id]),
      ),
    [data, downloadTracking],
  );
  const filteredModels = useMemo(
    () =>
      models.filter((model) => {
        if (category !== 'all' && inferModelCategory(model) !== category) {
          return false;
        }

        if (status !== 'all' && model.status !== status) {
          return false;
        }

        return true;
      }),
    [category, models, status],
  );
  const downloadedCount = useMemo(
    () => models.filter((model) => Boolean(model.local_path)).length,
    [models],
  );
  const pendingCount = useMemo(() => models.filter((model) => model.pending).length, [models]);
  const visibleModels = useMemo(
    () => filteredModels.slice(0, visibleCount),
    [filteredModels, visibleCount],
  );
  const hasMore = visibleModels.length < filteredModels.length;
  const canCreate = Boolean(createFile && !createModelPending && !importModelPackMutation.isPending);

  useEffect(() => {
    setVisibleCount(DEFAULT_VISIBLE_COUNT);
  }, [category, status]);

  useEffect(() => {
    if (!models.some((model) => model.pending)) {
      return;
    }

    const intervalId = window.setInterval(() => {
      void refetch();
    }, 3000);

    return () => window.clearInterval(intervalId);
  }, [models, refetch]);

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

  function loadMore() {
    setVisibleCount((current) =>
      current >= filteredModels.length
        ? current
        : Math.min(current + DEFAULT_VISIBLE_COUNT, filteredModels.length),
    );
  }

  async function createModel() {
    if (!createFile || createModelPending) {
      return;
    }

    setCreateModelPending(true);
    try {
      const created = await importModelPackMutation.mutateAsync({
        body: buildImportModelPackBody(createFile, t('pages.hub.error.onlySlabPacks')),
      });

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
      const task = await getModelDownloadTask(taskId);
      setModelDownloadTracking(modelId, {
        taskId,
        progress: normalizeTaskProgress(task.progress),
      });

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

      await sleep(MODEL_DOWNLOAD_POLL_INTERVAL_MS);
    }

    throw new Error(t('pages.hub.error.downloadTimedOut'));
  };

  const refreshCatalogAndFindModel = async (modelId: string) => {
    const refreshed = await refetch();
    const refreshedModels = toCatalogModelList(refreshed.data);
    return refreshedModels.find((model) => model.id === modelId);
  };

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
        description: getErrorMessage(downloadError),
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
        description: getErrorMessage(downloadError),
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
      void refetch();
    } catch (deleteError) {
      toast.error(t('pages.hub.toast.deleteFailed'), {
        description: getErrorMessage(deleteError),
      });
    }
  }

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
    refetch,
    canCreate,
    createModel,
    downloadModel,
    deleteModel,
    createModelPending: createModelPending || importModelPackMutation.isPending,
    deleteModelPending: deleteModelMutation.isPending,
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

function inferModelCategory(model: ModelItem): ModelCategory {
  const haystack = `${model.display_name} ${model.kind} ${model.backend_ids.join(' ')} ${model.repo_id} ${model.filename}`
    .toLowerCase()
    .trim();

  if (model.capabilities.includes('image_embedding') || haystack.includes('embed')) {
    return 'embedding';
  }

  if (
    model.capabilities.includes('image_generation') ||
    model.capabilities.includes('video_generation') ||
    haystack.includes('stable diffusion') ||
    haystack.includes('sdxl') ||
    haystack.includes('vision') ||
    haystack.includes('image')
  ) {
    return 'vision';
  }

  if (
    model.capabilities.includes('audio_transcription') ||
    model.capabilities.includes('audio_vad') ||
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

function buildImportModelPackBody(file: File, invalidFileMessage: string) {
  if (!isModelPackFile(file)) {
    throw new Error(invalidFileMessage);
  }

  const body = new FormData();
  body.set('file', file, file.name);
  return body;
}

function toModelItem(
  model: ReturnType<typeof toCatalogModelList>[number],
  tracking: DownloadTrackingState | undefined,
): ModelItem {
  const hasLocalPath = Boolean(model.local_path);
  const pending = !hasLocalPath && (model.pending || Boolean(tracking));
  const status = hasLocalPath ? 'ready' : pending ? 'downloading' : model.status;

  return {
    id: model.id,
    display_name: model.display_name,
    kind: model.kind,
    repo_id: model.repo_id,
    filename: model.filename,
    capabilities: model.capabilities,
    backend_ids: model.backend_ids,
    is_vad_model: modelSupportsCapability(model, 'audio_vad'),
    status,
    local_path: model.local_path,
    pending,
    download_task_id: tracking?.taskId ?? null,
    download_progress: tracking?.progress ?? null,
    updated_at: model.updated_at,
  };
}
