import { useDeferredValue, useEffect, useMemo, useState } from 'react';
import { toast } from 'sonner';

import api, { getErrorMessage } from '@/lib/api';
import { inferWhisperVadModel, toCatalogModelList, type CatalogModelStatus } from '@/lib/api/models';

const API_BASE_URL =
  (import.meta.env.VITE_API_BASE_URL as string | undefined) ?? 'http://localhost:3000';

export const PAGE_SIZE_OPTIONS = [10, 20, 50] as const;
export const STATUS_OPTIONS = [
  { value: 'all', label: 'All statuses' },
  { value: 'ready', label: 'Ready' },
  { value: 'downloading', label: 'Downloading' },
  { value: 'not_downloaded', label: 'Not downloaded' },
  { value: 'error', label: 'Error' },
] as const;
export const BACKEND_OPTIONS = [
  { id: 'ggml.llama', label: 'Llama', description: 'Text and chat models' },
  { id: 'ggml.whisper', label: 'Whisper', description: 'Speech and VAD models' },
  { id: 'ggml.diffusion', label: 'Diffusion', description: 'Image and video models' },
] as const;
export const EMPTY_FORM = {
  displayName: '',
  repoId: '',
  filename: '',
  backendIds: ['ggml.llama'],
};

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
export type CreateForm = typeof EMPTY_FORM;
export type AvailableFilesResponse = { repo_id: string; files: string[] };

export function useHubModelCatalog() {
  const [status, setStatus] = useState<ModelFilterStatus>('all');
  const [search, setSearch] = useState('');
  const [pageSize, setPageSize] = useState<PageSize>(10);
  const [page, setPage] = useState(1);
  const [isCreateOpen, setIsCreateOpen] = useState(false);
  const [form, setForm] = useState<CreateForm>(EMPTY_FORM);
  const [repoLookup, setRepoLookup] = useState<AvailableFilesResponse | null>(null);
  const [repoLookupFilter, setRepoLookupFilter] = useState('');
  const [repoLookupLoading, setRepoLookupLoading] = useState(false);
  const [repoLookupSearched, setRepoLookupSearched] = useState(false);
  const [modelToDelete, setModelToDelete] = useState<ModelItem | null>(null);

  const searchQuery = useDeferredValue(search).trim().toLowerCase();
  const repoFilterQuery = useDeferredValue(repoLookupFilter).trim().toLowerCase();

  const {
    data,
    error,
    isLoading,
    isRefetching,
    refetch,
  } = api.useQuery('get', '/v1/models');
  const createModelMutation = api.useMutation('post', '/v1/models');
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
        if (status !== 'all' && model.status !== status) {
          return false;
        }

        return matchesModel(model, searchQuery);
      }),
    [models, searchQuery, status],
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
  const repoFiles = useMemo(() => {
    const files = [...(repoLookup?.files ?? [])].sort((left, right) => left.localeCompare(right));
    if (!repoFilterQuery) {
      return files.slice(0, 200);
    }
    return files.filter((file) => file.toLowerCase().includes(repoFilterQuery)).slice(0, 200);
  }, [repoLookup?.files, repoFilterQuery]);

  const showingFrom = filteredModels.length === 0 ? 0 : (page - 1) * pageSize + 1;
  const showingTo = Math.min(page * pageSize, filteredModels.length);
  const canCreate = Boolean(
    !createModelMutation.isPending &&
      form.displayName.trim() &&
      form.repoId.trim() &&
      form.filename.trim() &&
      form.backendIds.length > 0,
  );

  useEffect(() => {
    setPage(1);
  }, [pageSize, searchQuery, status]);

  useEffect(() => {
    setPage((current) => Math.min(current, totalPages));
  }, [totalPages]);

  useEffect(() => {
    if (!repoLookup) {
      return;
    }

    if (repoLookup.repo_id !== form.repoId.trim()) {
      setRepoLookup(null);
      setRepoLookupFilter('');
      setRepoLookupSearched(false);
    }
  }, [form.repoId, repoLookup]);

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
    setForm(EMPTY_FORM);
    setRepoLookup(null);
    setRepoLookupFilter('');
    setRepoLookupLoading(false);
    setRepoLookupSearched(false);
  }

  function setCreateOpen(open: boolean) {
    setIsCreateOpen(open);
    if (!open && !createModelMutation.isPending) {
      resetCreateState();
    }
  }

  function setField<K extends keyof CreateForm>(key: K, value: CreateForm[K]) {
    setForm((current) => ({ ...current, [key]: value }));
  }

  function toggleBackend(id: string, checked: boolean) {
    setForm((current) => ({
      ...current,
      backendIds: checked
        ? Array.from(new Set([...current.backendIds, id]))
        : current.backendIds.filter((value) => value !== id),
    }));
  }

  function selectRepoFile(file: string) {
    setField('filename', file);
    if (!form.displayName.trim()) {
      setField('displayName', deriveDisplayName(file));
    }
  }

  async function searchRepoFiles() {
    const repoId = form.repoId.trim();
    if (!repoId) {
      toast.error('Enter a Hugging Face repo ID first.');
      return;
    }

    setRepoLookupLoading(true);
    setRepoLookupSearched(true);
    try {
      const url = new URL('/v1/models/available', API_BASE_URL);
      url.searchParams.set('repo_id', repoId);
      const response = await fetch(url.toString(), { method: 'GET' });
      if (!response.ok) {
        throw new Error((await response.text()) || `HTTP ${response.status}`);
      }

      setRepoLookup(parseAvailableFilesResponse(await response.json(), repoId));
    } catch (lookupError) {
      setRepoLookup(null);
      toast.error('Failed to search repo files.', {
        description:
          lookupError instanceof Error ? lookupError.message : 'Unknown error',
      });
    } finally {
      setRepoLookupLoading(false);
    }
  }

  async function createModel() {
    const backendIds = Array.from(new Set(form.backendIds));

    try {
      const results = await Promise.allSettled(
        backendIds.map((backendId) =>
          createModelMutation.mutateAsync({
            body: {
              display_name: form.displayName.trim(),
              provider: `local.${backendId}`,
              spec: {
                repo_id: form.repoId.trim(),
                filename: form.filename.trim(),
              },
            },
          }),
        ),
      );

      const created = results.flatMap((result) =>
        result.status === 'fulfilled' ? [result.value] : [],
      );
      const failed = results.flatMap((result) =>
        result.status === 'rejected' ? [result.reason] : [],
      );

      if (created.length === 0) {
        throw failed[0]?.reason ?? new Error('Unknown model creation error');
      }

      if (failed.length === 0) {
        toast.success(
          created.length === 1 ? 'Model added to catalog.' : 'Models added to catalog.',
          {
            description:
              created.length === 1
                ? created[0].display_name
                : `${created.length} backend-specific entries created.`,
          },
        );
      } else {
        toast.error('Added some model entries, but not every backend succeeded.', {
          description: `${created.length}/${backendIds.length} entries were created. ${getErrorMessage(
            failed[0],
          )}`,
        });
      }

      setStatus('all');
      setSearch(form.displayName.trim());
      setCreateOpen(false);
      void refetch();
    } catch (createError) {
      toast.error('Failed to add model.', {
        description: getErrorMessage(createError),
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
    status,
    setStatus,
    search,
    setSearch,
    pageSize,
    setPageSize,
    page,
    setPage,
    totalPages,
    isCreateOpen,
    setCreateOpen,
    form,
    setField,
    toggleBackend,
    selectRepoFile,
    repoLookup,
    repoLookupFilter,
    setRepoLookupFilter,
    repoLookupLoading,
    repoLookupSearched,
    repoFiles,
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
    searchRepoFiles,
    createModelPending: createModelMutation.isPending,
    deleteModelPending: deleteModelMutation.isPending,
  };
}

function matchesModel(model: ModelItem, query: string) {
  if (!query) {
    return true;
  }

  return [
    model.display_name,
    model.provider,
    model.repo_id,
    model.filename,
    model.status,
    model.local_path ?? '',
    ...model.backend_ids,
  ]
    .join(' ')
    .toLowerCase()
    .includes(query);
}

function deriveDisplayName(filename: string) {
  return (filename.split('/').at(-1) ?? filename)
    .replace(/\.(gguf|safetensors|bin|onnx)$/i, '')
    .replace(/[-_]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

function parseAvailableFilesResponse(payload: unknown, fallbackRepoId: string) {
  if (typeof payload !== 'object' || payload === null) {
    throw new Error('Invalid repo lookup response.');
  }

  const response = payload as { repo_id?: unknown; files?: unknown };
  if (!Array.isArray(response.files)) {
    throw new Error('Repo lookup response did not include a file list.');
  }

  return {
    repo_id: typeof response.repo_id === 'string' ? response.repo_id : fallbackRepoId,
    files: response.files.filter((value): value is string => typeof value === 'string'),
  };
}
