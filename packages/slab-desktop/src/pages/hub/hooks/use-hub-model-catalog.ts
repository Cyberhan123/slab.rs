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

export function useHubModelCatalog() {
  const [category, setCategory] = useState<ModelCategory>('all');
  const [status, setStatus] = useState<ModelFilterStatus>('all');
  const [visibleCount, setVisibleCount] = useState(DEFAULT_VISIBLE_COUNT);
  const [isCreateOpen, setIsCreateOpen] = useState(false);
  const [createFile, setCreateFile] = useState<File | null>(null);
  const [createModelPending, setCreateModelPending] = useState(false);
  const [modelToDelete, setModelToDelete] = useState<ModelItem | null>(null);

  const {
    data,
    error,
    isLoading,
    isRefetching,
    refetch,
  } = api.useQuery('get', '/v1/models');
  const deleteModelMutation = api.useMutation('delete', '/v1/models/{id}');

  const models = useMemo<ModelItem[]>(
    () =>
      toCatalogModelList(data)
        .map((model) => ({
          id: model.id,
          display_name: model.display_name,
          kind: model.kind,
          repo_id: model.repo_id,
          filename: model.filename,
          capabilities: model.capabilities,
          backend_ids: model.backend_ids,
          is_vad_model: modelSupportsCapability(model, 'audio_vad'),
          status: model.status,
          local_path: model.local_path,
          pending: model.pending,
          updated_at: model.updated_at,
        })),
    [data],
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
  const pendingCount = useMemo(
    () => models.filter((model) => model.status === 'downloading').length,
    [models],
  );
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
    if (!models.some((model) => model.status === 'downloading')) {
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

      toast.success('Model imported.', {
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
    deleteModel,
    createModelPending,
    deleteModelPending: deleteModelMutation.isPending,
  };
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
