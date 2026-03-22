import { useEffect, useMemo, useState } from 'react';
import { toast } from 'sonner';

import api, { getErrorMessage } from '@/lib/api';
import { inferWhisperVadModel, toCatalogModelList, type CatalogModelStatus } from '@/lib/api/models';

const API_BASE_URL =
  (import.meta.env.VITE_API_BASE_URL as string | undefined) ?? 'http://localhost:3000';

export const PAGE_SIZE_OPTIONS = [6, 12, 24] as const;
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
export type PageSize = (typeof PAGE_SIZE_OPTIONS)[number];
export type ModelItem = {
  id: string;
  display_name: string;
  provider: string;
  repo_id: string;
  filename: string;
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
  const [pageSize, setPageSize] = useState<PageSize>(6);
  const [page, setPage] = useState(1);
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
        .filter((model) => model.backend_id !== null)
        .map((model) => ({
          id: model.id,
          display_name: model.display_name,
          provider: model.provider,
          repo_id: model.repo_id,
          filename: model.filename,
          backend_ids: model.backend_ids,
          is_vad_model: model.backend_id === 'ggml.whisper' && inferWhisperVadModel(model),
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
  const totalPages = Math.max(1, Math.ceil(filteredModels.length / pageSize));
  const pagedModels = useMemo(() => {
    const start = (page - 1) * pageSize;
    return filteredModels.slice(start, start + pageSize);
  }, [filteredModels, page, pageSize]);

  const showingFrom = filteredModels.length === 0 ? 0 : (page - 1) * pageSize + 1;
  const showingTo = Math.min(page * pageSize, filteredModels.length);
  const canCreate = Boolean(createFile && !createModelPending);

  useEffect(() => {
    setPage(1);
  }, [category, pageSize, status]);

  useEffect(() => {
    setPage((current) => Math.min(current, totalPages));
  }, [totalPages]);

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

  async function createModel() {
    if (!createFile || createModelPending) {
      return;
    }

    setCreateModelPending(true);
    try {
      const payload = await readJsonFile(createFile);
      const created = await importModelConfig(payload);

      toast.success('Model config imported.', {
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
      toast.error('Failed to import model config.', {
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
    pageSize,
    setPageSize,
    page,
    setPage,
    totalPages,
    isCreateOpen,
    setCreateOpen,
    createFileName: createFile?.name ?? null,
    setCreateFile: updateCreateFile,
    modelToDelete,
    setModelToDelete,
    models,
    filteredModels,
    pagedModels,
    downloadedCount,
    pendingCount,
    showingFrom,
    showingTo,
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
  const haystack = `${model.display_name} ${model.provider} ${model.repo_id} ${model.filename}`
    .toLowerCase()
    .trim();

  if (haystack.includes('embed')) {
    return 'embedding';
  }

  if (
    model.backend_ids.includes('ggml.diffusion') ||
    haystack.includes('stable diffusion') ||
    haystack.includes('sdxl') ||
    haystack.includes('vision') ||
    haystack.includes('image')
  ) {
    return 'vision';
  }

  if (
    model.backend_ids.includes('ggml.whisper') ||
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

  return 'language';
}

async function readJsonFile(file: File): Promise<unknown> {
  const raw = await file.text();

  try {
    return JSON.parse(raw);
  } catch (error) {
    throw new Error(
      `Invalid JSON in ${file.name}: ${error instanceof Error ? error.message : 'Unknown parse error'}`,
    );
  }
}

async function importModelConfig(payload: unknown): Promise<ImportedModelResponse | null> {
  const response = await fetch(new URL('/v1/models/import', API_BASE_URL), {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(payload),
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
