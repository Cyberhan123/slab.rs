import { useEffect, useMemo, useState } from 'react';
import { toast } from 'sonner';

import api, { getErrorMessage } from '@/lib/api';
import { tauriAwareFetch } from '@/lib/api/tauri-transport';
import {
  modelSupportsCapability,
  toCatalogModelList,
  type CatalogModelStatus,
  type ModelCapability,
} from '@/lib/api/models';
import { SERVER_BASE_URL } from '@/lib/config';

const DEFAULT_VISIBLE_COUNT = 10;
const MODEL_DOWNLOAD_POLL_INTERVAL_MS = 2_000;
const MODEL_DOWNLOAD_TIMEOUT_MS = 30 * 60 * 1_000;
export const CATEGORY_OPTIONS = [
  { value: 'all', label: 'All models' },
  { value: 'language', label: 'Large language' },
  { value: 'vision', label: 'Vision' },
  { value: 'audio', label: 'Audio' },
  { value: 'coding', label: 'Coding' },
  { value: 'embedding', label: 'Embedding' },
] as const;
export const STATUS_OPTIONS = [
  { value: 'all', label: 'All statuses' },
  { value: 'ready', label: 'Ready' },
  { value: 'downloading', label: 'Downloading' },
  { value: 'not_downloaded', label: 'Not downloaded' },
  { value: 'error', label: 'Error' },
] as const;

export type ModelCategory = (typeof CATEGORY_OPTIONS)[number]['value'];
export type ModelFilterStatus = (typeof STATUS_OPTIONS)[number]['value'];
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
  updated_at: string;
};

type ImportedModelResponse = {
  display_name?: string;
};

type TaskStatusResponse = {
  status: string;
  error_msg?: string | null;
};

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

export function useHubModelCatalog() {
  const [category, setCategory] = useState<ModelCategory>('all');
  const [status, setStatus] = useState<ModelFilterStatus>('all');
  const [visibleCount, setVisibleCount] = useState(DEFAULT_VISIBLE_COUNT);
  const [isCreateOpen, setIsCreateOpen] = useState(false);
  const [createFile, setCreateFile] = useState<File | null>(null);
  const [createModelPending, setCreateModelPending] = useState(false);
  const [modelToDelete, setModelToDelete] = useState<ModelItem | null>(null);
  const [modelToEnhance, setModelToEnhance] = useState<ModelItem | null>(null);
  const [activeDownloadTasks, setActiveDownloadTasks] = useState<Record<string, string>>({});

  const {
    data,
    error,
    isLoading,
    isRefetching,
    refetch,
  } = api.useQuery('get', '/v1/models');
  const downloadModelMutation = api.useMutation('post', '/v1/models/download');
  const deleteModelMutation = api.useMutation('delete', '/v1/models/{id}');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');

  const models = useMemo<ModelItem[]>(
    () =>
      toCatalogModelList(data)
        .map((model) => {
          const pending = model.pending || Boolean(activeDownloadTasks[model.id]);

          return {
            id: model.id,
            display_name: model.display_name,
            kind: model.kind,
            repo_id: model.repo_id,
            filename: model.filename,
            capabilities: model.capabilities,
            backend_ids: model.backend_ids,
            is_vad_model: modelSupportsCapability(model, 'audio_vad'),
            status: pending ? 'downloading' : model.status,
            local_path: model.local_path,
            pending,
            updated_at: model.updated_at,
          };
        }),
    [activeDownloadTasks, data],
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
  const canCreate = Boolean(createFile && !createModelPending);

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
      const created = await importModelFile(createFile);

      toast.success('Model imported to catalog.', {
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
      toast.error('Failed to import model.', {
        description: getErrorMessage(createError),
      });
    } finally {
      setCreateModelPending(false);
    }
  }

  const waitForTaskToFinish = async (taskId: string) => {
    const deadline = Date.now() + MODEL_DOWNLOAD_TIMEOUT_MS;

    while (Date.now() < deadline) {
      const task = (await getTaskMutation.mutateAsync({
        params: { path: { id: taskId } },
      })) as TaskStatusResponse;

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
    const refreshed = await refetch();
    const refreshedModels = toCatalogModelList(refreshed.data);
    return refreshedModels.find((model) => model.id === modelId);
  };

  async function trackModelDownload(model: ModelItem, taskId: string) {
    try {
      await waitForTaskToFinish(taskId);

      const refreshedModel = await refreshCatalogAndFindModel(model.id);
      if (!refreshedModel?.local_path) {
        throw new Error('Model download completed, but local_path is empty');
      }

      toast.success('Model downloaded.', {
        description: model.display_name,
      });
    } catch (downloadError) {
      toast.error('Model download failed.', {
        description: getErrorMessage(downloadError),
      });
    } finally {
      setActiveDownloadTasks((current) => {
        if (!current[model.id]) {
          return current;
        }

        const next = { ...current };
        delete next[model.id];
        return next;
      });
      void refetch();
    }
  }

  async function downloadModel(model: ModelItem) {
    if (!canDownloadModel(model) || activeDownloadTasks[model.id]) {
      return;
    }

    try {
      const response = await downloadModelMutation.mutateAsync({
        body: {
          model_id: model.id,
        },
      });
      const taskId = extractTaskId(response);

      if (!taskId) {
        throw new Error('Failed to start model download task');
      }

      setActiveDownloadTasks((current) => ({
        ...current,
        [model.id]: taskId,
      }));
      toast.success('Download started.', {
        description: model.display_name,
      });
      void trackModelDownload(model, taskId);
    } catch (downloadError) {
      toast.error('Failed to start download.', {
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

      toast.success('Model removed from catalog.', {
        description: modelToDelete.display_name,
      });
      setModelToDelete(null);
      void refetch();
    } catch (deleteError) {
      toast.error('Failed to delete model.', {
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
    createModelPending,
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

async function importModelFile(file: File): Promise<ImportedModelResponse | null> {
  if (!isModelPackFile(file)) {
    throw new Error('Only .slab model packs are supported.');
  }

  return importModelPack(file);
}

async function importModelPack(file: File): Promise<ImportedModelResponse | null> {
  const body = new FormData();
  body.set('file', file, file.name);

  const response = await tauriAwareFetch(new URL('/v1/models/import-pack', `${SERVER_BASE_URL}/`), {
    method: 'POST',
    body,
  });

  const raw = await response.text();
  if (!response.ok) {
    throw new Error(parseApiError(raw, response.status));
  }

  if (!raw.trim()) {
    return null;
  }

  return parseImportedModelResponse(raw);
}

function parseImportedModelResponse(raw: string): ImportedModelResponse | null {
  try {
    const payload = JSON.parse(raw);
    if (typeof payload !== 'object' || payload === null) {
      return null;
    }

    return payload as ImportedModelResponse;
  } catch {
    return null;
  }
}

function extractTaskId(payload: unknown): string | null {
  if (typeof payload !== 'object' || payload === null) {
    return null;
  }

  const taskId =
    (payload as { operation_id?: unknown }).operation_id ??
    (payload as { task_id?: unknown }).task_id;

  if (typeof taskId !== 'string') {
    return null;
  }

  const trimmed = taskId.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function parseApiError(raw: string, status: number) {
  if (!raw.trim()) {
    return `HTTP ${status}`;
  }

  try {
    const payload = JSON.parse(raw) as { message?: unknown };
    if (typeof payload.message === 'string' && payload.message.trim()) {
      return payload.message;
    }
  } catch {
    // Ignore JSON parse failures and fall back to the raw response body.
  }

  return raw;
}
