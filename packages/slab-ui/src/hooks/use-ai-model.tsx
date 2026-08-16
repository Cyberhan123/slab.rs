import { useCallback, useEffect, useMemo, useState } from 'react';

import api from '@slab/api';
import type { components } from '@slab/api/v1';
import { useHeaderUiStore } from '@slab/ui/store/useHeaderUiStore';
import {
  extractTaskId,
  isFailedTaskStatus,
  MODEL_DOWNLOAD_POLL_INTERVAL_MS,
  MODEL_DOWNLOAD_TIMEOUT_MS,
  sleep,
} from '@slab/ui/pages/task/utils';

type UnknownRecord = Record<string, unknown>;

export type UnifiedModelResponse = components['schemas']['UnifiedModelResponse'];
export type ModelCapability = components['schemas']['ModelCapability'];
export type ChatModelCapabilities = components['schemas']['ChatModelCapabilities'];
export type AiModelStatus = 'ready' | 'not_downloaded' | 'downloading' | 'error';
export type AiModelRuntimeState = components['schemas']['ModelRuntimeStateResponse'];

export type AiModel = Omit<UnifiedModelResponse, 'status'> & {
  status: AiModelStatus;
  backend_id: string | null;
  backend_ids: string[];
  capabilities: ModelCapability[];
  chat_capabilities: ChatModelCapabilities | null;
  repo_id: string;
  filename: string;
  local_path: string | null;
  pending: boolean;
  runtime_state: AiModelRuntimeState | null;
  size_bytes: number | null;
};

export type AiModelOption = {
  id: string;
  label: string;
  disabled?: boolean;
  downloaded: boolean;
  pending: boolean;
  local_path: string | null;
  source: AiModel['kind'];
};

export type UseAiModelOptions = {
  capability?: ModelCapability;
  storageKey?: string;
  localOnly?: boolean;
  includeCloud?: boolean;
  isOptionDisabled?: (model: AiModel) => boolean;
  getDefaultModelId?: (models: AiModel[], options: AiModelOption[]) => string | undefined;
};

export type EnsureDownloadedResult = {
  model: AiModel;
  modelPath: string | null;
  downloadedNow: boolean;
};

export type EnsureLoadedResult = EnsureDownloadedResult & {
  loadedNow: boolean;
  runtimeStatus: components['schemas']['ModelStatusResponse'] | null;
};

export type UseAiModelResult = {
  models: AiModel[];
  localModels: AiModel[];
  options: AiModelOption[];
  selectedId: string;
  setSelectedId: (value: string) => void;
  selected: AiModel | undefined;
  loading: boolean;
  refetching: boolean;
  error: unknown;
  refetch: () => Promise<{ data: unknown }>;
  status: {
    downloading: boolean;
    loading: boolean;
    switching: boolean;
    unloading: boolean;
    busy: boolean;
  };
  download: (modelId: string) => Promise<unknown>;
  ensureDownloaded: (modelId: string, options?: { forceDownload?: boolean }) => Promise<EnsureDownloadedResult>;
  load: (modelId: string) => Promise<unknown>;
  switchTo: (modelId: string) => Promise<unknown>;
  unload: (modelId: string) => Promise<unknown>;
  ensureLoaded: (modelId: string, options?: { forceDownload?: boolean }) => Promise<EnsureLoadedResult>;
};

export function normalizeModelStatus(status: string): AiModelStatus {
  switch (status) {
    case 'ready':
    case 'not_downloaded':
    case 'downloading':
    case 'error':
      return status;
    default:
      return 'error';
  }
}

export function normalizeAiModel(model: UnifiedModelResponse): AiModel {
  const backendId = model.backend_id ?? null;
  const status = normalizeModelStatus(model.status);
  const localPath = model.spec.local_path ?? null;
  const chatCapabilities =
    model.chat_capabilities && isChatModelCapabilities(model.chat_capabilities)
      ? model.chat_capabilities
      : null;
  const runtimeState = isAiModelRuntimeState(model.runtime_state) ? model.runtime_state : null;
  const sizeBytes =
    typeof (model as UnknownRecord).size_bytes === 'number'
      ? ((model as UnknownRecord).size_bytes as number)
      : null;

  return {
    ...model,
    status,
    backend_id: backendId,
    backend_ids: backendId ? [backendId] : [],
    capabilities: model.capabilities,
    chat_capabilities: chatCapabilities,
    repo_id: model.spec.repo_id ?? '',
    filename: model.spec.filename ?? '',
    local_path: localPath,
    pending: status === 'downloading',
    runtime_state: runtimeState,
    size_bytes: sizeBytes,
  };
}

export function modelSupportsCapability(
  model: Pick<AiModel, 'capabilities'>,
  capability: ModelCapability,
): boolean {
  return model.capabilities.includes(capability);
}

export function toAiModelList(payload: unknown): AiModel[] {
  return toUnifiedModelList(payload).map(normalizeAiModel);
}

export function useAiModel({
  capability,
  storageKey,
  localOnly = false,
  includeCloud = false,
  isOptionDisabled,
  getDefaultModelId,
}: UseAiModelOptions = {}): UseAiModelResult {
  const hasHydrated = useHeaderUiStore((state) => state.hasHydrated);
  const persistedSelectedId = useHeaderUiStore((state) =>
    storageKey ? state.selections[storageKey] ?? '' : '',
  );
  const setSelection = useHeaderUiStore((state) => state.setSelection);
  const clearSelection = useHeaderUiStore((state) => state.clearSelection);
  const [localSelectedId, setLocalSelectedId] = useState('');
  const [loadedModelId, setLoadedModelId] = useState<string | null>(null);

  const {
    data,
    error,
    isLoading,
    isRefetching,
    refetch: refetchModels,
  } = api.useQuery('get', '/v1/models', {
    params: capability
      ? {
          query: {
            capability,
          },
        }
      : undefined,
  });
  const downloadModelMutation = api.useMutation('post', '/v1/models/download', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const loadModelMutation = api.useMutation('post', '/v1/models/load', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const switchModelMutation = api.useMutation('post', '/v1/models/switch', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const unloadModelMutation = api.useMutation('post', '/v1/models/unload', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });

  const models = useMemo(() => {
    const normalized = toAiModelList(data);

    if (localOnly) {
      return normalized.filter((model) => model.kind === 'local');
    }

    if (!includeCloud && capability) {
      return normalized.filter((model) => model.kind === 'local');
    }

    return normalized;
  }, [capability, data, includeCloud, localOnly]);
  const localModels = useMemo(() => models.filter((model) => model.kind === 'local'), [models]);
  const options = useMemo<AiModelOption[]>(
    () =>
      models.map((model) => ({
        id: model.id,
        label: model.display_name,
        disabled: isOptionDisabled?.(model) ?? (model.kind === 'cloud' ? !includeCloud && localOnly : false),
        downloaded: model.kind === 'cloud' || Boolean(model.local_path),
        pending: model.pending,
        local_path: model.local_path,
        source: model.kind,
      })),
    [includeCloud, isOptionDisabled, localOnly, models],
  );
  const selectedId = storageKey ? persistedSelectedId : localSelectedId;
  const selected = useMemo(
    () => models.find((model) => model.id === selectedId),
    [models, selectedId],
  );
  const setSelectedId = useCallback(
    (value: string) => {
      if (storageKey) {
        setSelection(storageKey, value);
        return;
      }

      setLocalSelectedId(value.trim());
    },
    [setSelection, storageKey],
  );

  useEffect(() => {
    if (storageKey && !hasHydrated) {
      return;
    }

    if (isLoading) {
      return;
    }

    const enabledOptions = options.filter((option) => !option.disabled);
    if (enabledOptions.length === 0) {
      if (selectedId) {
        if (storageKey) {
          clearSelection(storageKey);
        } else {
          setLocalSelectedId('');
        }
      }
      return;
    }

    if (enabledOptions.some((option) => option.id === selectedId)) {
      return;
    }

    const preferredValue = getDefaultModelId?.(models, options) ?? '';
    const fallbackValue = enabledOptions.some((option) => option.id === preferredValue)
      ? preferredValue
      : enabledOptions[0]?.id ?? '';

    if (!fallbackValue) {
      if (storageKey) {
        clearSelection(storageKey);
      } else {
        setLocalSelectedId('');
      }
      return;
    }

    if (storageKey) {
      setSelection(storageKey, fallbackValue);
    } else {
      setLocalSelectedId(fallbackValue);
    }
  }, [
    clearSelection,
    getDefaultModelId,
    hasHydrated,
    isLoading,
    models,
    options,
    selectedId,
    setSelection,
    storageKey,
  ]);

  const refetch = useCallback(() => refetchModels(), [refetchModels]);

  const waitForTaskToFinish = useCallback(
    async (taskId: string) => {
      const deadline = Date.now() + MODEL_DOWNLOAD_TIMEOUT_MS;

      while (Date.now() < deadline) {
        // eslint-disable-next-line no-await-in-loop
        const task = (await getTaskMutation.mutateAsync({
          params: { path: { id: taskId } },
        })) as { status: string; error_msg?: string | null };

        if (task.status === 'succeeded') {
          return;
        }

        if (isFailedTaskStatus(task.status)) {
          throw new Error(task.error_msg ?? `Task ${taskId} ended with status: ${task.status}`);
        }

        // eslint-disable-next-line no-await-in-loop
        await sleep(MODEL_DOWNLOAD_POLL_INTERVAL_MS);
      }

      throw new Error(`Timed out waiting for model download task ${taskId}.`);
    },
    [getTaskMutation],
  );

  const refreshAndFindModel = useCallback(
    async (modelId: string) => {
      const refreshed = await refetchModels();
      return toAiModelList(refreshed.data).find((model) => model.id === modelId);
    },
    [refetchModels],
  );

  const findCurrentModel = useCallback(
    async (modelId: string) => models.find((model) => model.id === modelId) ?? refreshAndFindModel(modelId),
    [models, refreshAndFindModel],
  );

  const download = useCallback(
    (modelId: string) =>
      downloadModelMutation.mutateAsync({
        body: {
          model_id: modelId,
        },
      }),
    [downloadModelMutation],
  );

  const ensureDownloaded = useCallback(
    async (
      modelId: string,
      { forceDownload = false }: { forceDownload?: boolean } = {},
    ): Promise<EnsureDownloadedResult> => {
      const model = await findCurrentModel(modelId);
      if (!model) {
        throw new Error(`Selected model '${modelId}' is not available.`);
      }

      if (model.kind === 'cloud') {
        return {
          model,
          modelPath: null,
          downloadedNow: false,
        };
      }

      if (model.local_path && !forceDownload) {
        return {
          model,
          modelPath: model.local_path,
          downloadedNow: false,
        };
      }

      const downloadResponse = await download(modelId);
      const taskId = extractTaskId(downloadResponse);
      if (!taskId) {
        throw new Error(`Failed to start model download task for '${modelId}'.`);
      }

      await waitForTaskToFinish(taskId);

      const refreshedModel = await refreshAndFindModel(modelId);
      if (!refreshedModel?.local_path) {
        throw new Error(`Model '${modelId}' download completed, but local_path is empty.`);
      }

      return {
        model: refreshedModel,
        modelPath: refreshedModel.local_path,
        downloadedNow: true,
      };
    },
    [download, findCurrentModel, refreshAndFindModel, waitForTaskToFinish],
  );

  const load = useCallback(
    async (modelId: string) => {
      const model = await findCurrentModel(modelId);
      if (model?.kind === 'cloud') {
        return model;
      }

      const result = await loadModelMutation.mutateAsync({
        body: {
          model_id: modelId,
        },
      });
      setLoadedModelId(modelId);
      return result;
    },
    [findCurrentModel, loadModelMutation],
  );

  const switchTo = useCallback(
    async (modelId: string) => {
      const model = await findCurrentModel(modelId);
      if (model?.kind === 'cloud') {
        setLoadedModelId(null);
        return model;
      }

      const result = await switchModelMutation.mutateAsync({
        body: {
          model_id: modelId,
        },
      });
      setLoadedModelId(modelId);
      return result;
    },
    [findCurrentModel, switchModelMutation],
  );

  const unload = useCallback(
    async (modelId: string) => {
      const result = await unloadModelMutation.mutateAsync({
        body: {
          model_id: modelId,
        },
      });
      setLoadedModelId((current) => (current === modelId ? null : current));
      return result;
    },
    [unloadModelMutation],
  );

  const ensureLoaded = useCallback(
    async (
      modelId: string,
      { forceDownload = false }: { forceDownload?: boolean } = {},
    ): Promise<EnsureLoadedResult> => {
      const downloaded = await ensureDownloaded(modelId, { forceDownload });
      if (downloaded.model.kind === 'cloud') {
        setLoadedModelId(null);
        return {
          ...downloaded,
          loadedNow: false,
          runtimeStatus: null,
        };
      }

      if (loadedModelId === modelId) {
        return {
          ...downloaded,
          loadedNow: false,
          runtimeStatus: null,
        };
      }

      let runtimeStatus: components['schemas']['ModelStatusResponse'];
      if (loadedModelId) {
        runtimeStatus = (await switchTo(modelId)) as components['schemas']['ModelStatusResponse'];
      } else {
        runtimeStatus = (await load(modelId)) as components['schemas']['ModelStatusResponse'];
      }

      return {
        ...downloaded,
        loadedNow: true,
        runtimeStatus,
      };
    },
    [ensureDownloaded, load, loadedModelId, switchTo],
  );

  const status = {
    downloading: downloadModelMutation.isPending,
    loading: loadModelMutation.isPending,
    switching: switchModelMutation.isPending,
    unloading: unloadModelMutation.isPending,
    busy:
      downloadModelMutation.isPending ||
      loadModelMutation.isPending ||
      switchModelMutation.isPending ||
      unloadModelMutation.isPending,
  };

  return {
    models,
    localModels,
    options,
    selectedId,
    setSelectedId,
    selected,
    loading: isLoading,
    refetching: isRefetching,
    error,
    refetch,
    status,
    download,
    ensureDownloaded,
    load,
    switchTo,
    unload,
    ensureLoaded,
  };
}

function toUnifiedModelList(payload: unknown): UnifiedModelResponse[] {
  return Array.isArray(payload)
    ? payload.filter((item): item is UnifiedModelResponse => isUnifiedModelResponse(item))
    : [];
}

function isUnifiedModelResponse(value: unknown): value is UnifiedModelResponse {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const model = value as UnknownRecord;
  if (
    typeof model.id !== 'string' ||
    typeof model.display_name !== 'string' ||
    (model.kind !== 'local' && model.kind !== 'cloud') ||
    typeof model.status !== 'string' ||
    typeof model.created_at !== 'string' ||
    typeof model.updated_at !== 'string'
  ) {
    return false;
  }

  if (typeof model.spec !== 'object' || model.spec === null || Array.isArray(model.spec)) {
    return false;
  }

  const spec = model.spec as UnknownRecord;
  return (
    isOptionalString(model.backend_id) &&
    isCapabilityList(model.capabilities) &&
    isOptionalChatModelCapabilities(model.chat_capabilities) &&
    isOptionalAiModelRuntimeState(model.runtime_state) &&
    isOptionalNumber(model.size_bytes) &&
    isOptionalString(spec.provider_id) &&
    isOptionalString(spec.remote_model_id) &&
    isOptionalString(spec.repo_id) &&
    isOptionalString(spec.filename) &&
    isOptionalString(spec.local_path)
  );
}

function isOptionalNumber(value: unknown): boolean {
  return value === undefined || value === null || typeof value === 'number';
}

function isOptionalAiModelRuntimeState(
  value: unknown,
): value is AiModelRuntimeState | null | undefined {
  return value === undefined || value === null || isAiModelRuntimeState(value);
}

function isAiModelRuntimeState(value: unknown): value is AiModelRuntimeState {
  return (
    typeof value === 'object' &&
    value !== null &&
    !Array.isArray(value) &&
    typeof (value as UnknownRecord).backend_id === 'string' &&
    typeof (value as UnknownRecord).loaded === 'boolean' &&
    typeof (value as UnknownRecord).active === 'boolean' &&
    typeof (value as UnknownRecord).active_refs === 'number'
  );
}

function isCapabilityList(value: unknown): value is ModelCapability[] {
  return Array.isArray(value) && value.every((capability) => isModelCapability(capability));
}

function isModelCapability(value: unknown): value is ModelCapability {
  switch (value) {
    case 'text_generation':
    case 'audio_transcription':
    case 'image_generation':
    case 'image_embedding':
    case 'chat_generation':
    case 'audio_vad':
    case 'video_generation':
      return true;
    default:
      return false;
  }
}

function isOptionalChatModelCapabilities(
  value: unknown,
): value is ChatModelCapabilities | null | undefined {
  return value === undefined || value === null || isChatModelCapabilities(value);
}

function isChatModelCapabilities(value: unknown): value is ChatModelCapabilities {
  return (
    typeof value === 'object' &&
    value !== null &&
    !Array.isArray(value) &&
    typeof (value as UnknownRecord).raw_gbnf === 'boolean' &&
    typeof (value as UnknownRecord).structured_output === 'boolean' &&
    typeof (value as UnknownRecord).reasoning_controls === 'boolean'
  );
}

function isOptionalString(value: unknown): boolean {
  return value === undefined || value === null || typeof value === 'string';
}
