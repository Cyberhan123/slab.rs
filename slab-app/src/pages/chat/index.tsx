import { XProvider } from '@ant-design/x';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { toast } from 'sonner';
import api from '@/lib/api';
import '@ant-design/x-markdown/themes/light.css';
import '@ant-design/x-markdown/themes/dark.css';
import { useMarkdownTheme } from './hooks/use-markdowm-theme';
import locale from './local';
import {
  API_BASE_URL,
  ChatContext,
  DEFAULT_CONVERSATIONS_ITEMS,
  DEFAULT_CONVERSATION_KEY,
} from './chat-context';
import { useStyle } from './hooks/use-style';
import { ChatSidebar } from './components/chat-sidebar';
import { ChatMessageList } from './components/chat-message-list';
import { ChatInput } from './components/chat-input';
import { useChat } from './hooks/use-chat';
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';

const LLAMA_BACKEND_ID = 'ggml.llama';
const MODEL_DOWNLOAD_POLL_INTERVAL_MS = 2_000;
const MODEL_DOWNLOAD_TIMEOUT_MS = 30 * 60 * 1_000;

type ModelOptionSource = 'local' | 'cloud';

type ModelOption = {
  id: string;
  label: string;
  downloaded: boolean;
  pending: boolean;
  source: ModelOptionSource;
};

type ChatModelApiItem = {
  id: string;
  display_name: string;
  source: ModelOptionSource;
  downloaded: boolean;
  pending: boolean;
  provider_name?: string | null;
};

type UnifiedModelApiItem = {
  id: string;
  display_name: string;
  provider: string;
  status: string;
  spec?: {
    local_path?: string | null;
  } | null;
};

function isChatModelApiItem(value: unknown): value is ChatModelApiItem {
  if (typeof value !== 'object' || value === null) return false;
  const obj = value as Record<string, unknown>;
  return (
    typeof obj.id === 'string' &&
    typeof obj.display_name === 'string' &&
    (obj.source === 'local' || obj.source === 'cloud') &&
    typeof obj.downloaded === 'boolean' &&
    typeof obj.pending === 'boolean'
  );
}

function isUnifiedModelApiItem(value: unknown): value is UnifiedModelApiItem {
  if (typeof value !== 'object' || value === null) return false;
  const obj = value as Record<string, unknown>;
  if (
    typeof obj.id !== 'string' ||
    typeof obj.display_name !== 'string' ||
    typeof obj.provider !== 'string' ||
    typeof obj.status !== 'string'
  ) {
    return false;
  }

  if (obj.spec === undefined || obj.spec === null) {
    return true;
  }

  if (typeof obj.spec !== 'object' || Array.isArray(obj.spec)) {
    return false;
  }

  const spec = obj.spec as Record<string, unknown>;
  return (
    spec.local_path === undefined ||
    spec.local_path === null ||
    typeof spec.local_path === 'string'
  );
}

function toUnifiedModelList(payload: unknown): UnifiedModelApiItem[] {
  return Array.isArray(payload)
    ? payload.filter((item): item is UnifiedModelApiItem => isUnifiedModelApiItem(item))
    : [];
}

function Chat() {
  const [className] = useMarkdownTheme();
  const styles = useStyle();
  const [deepThink, setDeepThink] = useState<boolean>(true);
  const [curConversation, setCurConversation] = useState<string>(
    DEFAULT_CONVERSATIONS_ITEMS[0]?.key ?? DEFAULT_CONVERSATION_KEY,
  );

  const [selectedModelId, setSelectedModelId] = useState('');
  const [loadedModelId, setLoadedModelId] = useState<string | null>(null);
  const [cloudModelOptions, setCloudModelOptions] = useState<ModelOption[]>([]);
  const [cloudModelsLoading, setCloudModelsLoading] = useState(false);

  usePageHeader(PAGE_HEADER_META.chat);

  const {
    data: catalogModels,
    isLoading: catalogModelsLoading,
    refetch: refetchCatalogModels,
  } = api.useQuery('get', '/v1/models');

  const downloadModelMutation = api.useMutation('post', '/v1/models/download');
  const loadModelMutation = api.useMutation('post', '/v1/models/load');
  const switchModelMutation = api.useMutation('post', '/v1/models/switch');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');

  const loadCloudModels = useCallback(async () => {
    setCloudModelsLoading(true);
    try {
      const response = await fetch(`${API_BASE_URL}/v1/chat/models`, {
        method: 'GET',
      });
      if (!response.ok) {
        const detail = await response.text();
        throw new Error(`HTTP ${response.status}: ${detail || 'failed to load models'}`);
      }

      const payload: unknown = await response.json();
      if (!Array.isArray(payload)) {
        throw new Error('Invalid chat model payload');
      }

      const cloudOnly = payload
        .filter((item): item is ChatModelApiItem => isChatModelApiItem(item))
        .filter((item) => item.source === 'cloud')
        .map<ModelOption>((item) => ({
          id: item.id,
          label: item.provider_name
            ? `${item.provider_name} / ${item.display_name}`
            : item.display_name,
          downloaded: item.downloaded,
          pending: item.pending,
          source: 'cloud',
        }));

      setCloudModelOptions(cloudOnly);
    } catch (error: any) {
      setCloudModelOptions([]);
      toast.error('Failed to load cloud model options', {
        description: error?.message || 'Unknown error',
      });
    } finally {
      setCloudModelsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadCloudModels();
  }, [loadCloudModels]);

  const parsedCatalogModels = useMemo(
    () => toUnifiedModelList(catalogModels),
    [catalogModels],
  );

  const llamaModels = useMemo(
    () =>
      parsedCatalogModels.filter(
        (model) => model.provider === `local.${LLAMA_BACKEND_ID}`,
      ),
    [parsedCatalogModels],
  );

  const localModelOptions = useMemo<ModelOption[]>(
    () =>
      llamaModels.map((model) => ({
        id: model.id,
        label: model.display_name,
        downloaded: Boolean(model.spec?.local_path),
        pending: model.status === 'downloading',
        source: 'local',
      })),
    [llamaModels],
  );

  const modelOptions = useMemo<ModelOption[]>(
    () => [...localModelOptions, ...cloudModelOptions],
    [localModelOptions, cloudModelOptions],
  );

  useEffect(() => {
    if (modelOptions.length === 0) {
      setSelectedModelId('');
      return;
    }

    const exists = modelOptions.some((model) => model.id === selectedModelId);
    if (!selectedModelId || !exists) {
      setSelectedModelId(modelOptions[0].id);
    }
  }, [modelOptions, selectedModelId]);

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
    const models = toUnifiedModelList(refreshed.data);
    return models.find((model) => model.id === modelId);
  };

  const ensureDownloadedModelPath = async (
    modelId: string,
    forceDownload = false,
  ): Promise<{ modelPath: string; downloadedNow: boolean }> => {
    let model = llamaModels.find((item) => item.id === modelId);
    if (!model) {
      model = await refreshCatalogAndFindModel(modelId);
    }

    if (!model) {
      throw new Error('Selected model does not exist in catalog');
    }

    if (model.provider !== `local.${LLAMA_BACKEND_ID}`) {
      throw new Error(`Selected model does not support ${LLAMA_BACKEND_ID}`);
    }

    if (model.spec?.local_path && !forceDownload) {
      return { modelPath: model.spec.local_path, downloadedNow: false };
    }

    const downloadResponse = await downloadModelMutation.mutateAsync({
      body: {
        backend_id: LLAMA_BACKEND_ID,
        model_id: modelId,
      },
    });
    const taskId = extractTaskId(downloadResponse);

    if (!taskId) {
      throw new Error('Failed to start model download task');
    }

    await waitForTaskToFinish(taskId);

    const refreshedModel = await refreshCatalogAndFindModel(modelId);
    if (!refreshedModel?.spec?.local_path) {
      throw new Error('Model download completed, but local_path is empty');
    }

    return { modelPath: refreshedModel.spec.local_path, downloadedNow: true };
  };

  const loadOrSwitchSelectedModel = async (modelPath: string) => {
    const shouldSwitch = Boolean(loadedModelId && loadedModelId !== selectedModelId);
    if (shouldSwitch) {
      await switchModelMutation.mutateAsync({
        body: {
          backend_id: LLAMA_BACKEND_ID,
          model_path: modelPath,
        },
      });
      return;
    }

    await loadModelMutation.mutateAsync({
      body: {
        backend_id: LLAMA_BACKEND_ID,
        model_path: modelPath,
      },
    });
  };

  const prepareSelectedModel = async () => {
    if (!selectedModelId) {
      throw new Error('Please select a chat model first.');
    }

    if (loadedModelId === selectedModelId) {
      return;
    }

    const selectedOption = modelOptions.find((item) => item.id === selectedModelId);
    if (!selectedOption) {
      throw new Error('Selected model is not available');
    }

    if (selectedOption.source === 'cloud') {
      setLoadedModelId(selectedModelId);
      return;
    }

    const selectedLocal = llamaModels.find((item) => item.id === selectedModelId);
    const { modelPath, downloadedNow } = await ensureDownloadedModelPath(selectedModelId);

    if (downloadedNow) {
      toast.success(`Downloaded ${selectedLocal?.display_name ?? selectedModelId}`);
    }

    try {
      await loadOrSwitchSelectedModel(modelPath);
    } catch (firstLoadError) {
      // If catalog says "downloaded" but loading fails, local cache may be stale/corrupted.
      if (downloadedNow) {
        throw firstLoadError;
      }

      toast.message('Model load failed, re-downloading and retrying once...');

      const retry = await ensureDownloadedModelPath(selectedModelId, true);
      if (retry.downloadedNow) {
        toast.success(`Downloaded ${selectedLocal?.display_name ?? selectedModelId}`);
      }

      await loadOrSwitchSelectedModel(retry.modelPath);
    }

    setLoadedModelId(selectedModelId);
  };

  const ensureChatModelReady = async () => {
    try {
      await prepareSelectedModel();
    } catch (err: any) {
      toast.error('Failed to prepare chat model.', {
        description: err?.message || err?.error || 'Unknown error',
      });
      throw err;
    }
  };

  const {
    messages,
    isRequesting,
    abort,
    onReload,
    activeConversation,
    handleSubmit,
  } = useChat(curConversation, selectedModelId || 'slab-llama', deepThink, ensureChatModelReady);

  const isPreparingModel =
    loadModelMutation.isPending ||
    switchModelMutation.isPending ||
    downloadModelMutation.isPending;
  const modelLoading = catalogModelsLoading || cloudModelsLoading;

  return (
    <XProvider locale={locale}>
      <ChatContext.Provider value={{ onReload }}>
        <div className={styles.layout}>
          <ChatSidebar
            curConversation={curConversation}
            setCurConversation={setCurConversation}
            activeConversation={activeConversation}
            messages={messages}
          />
          <div className={styles.chat}>
            <div className={styles.chatList}>
              {messages?.length !== 0 ? (
                <ChatMessageList
                  messages={messages}
                  className={className}
                  onReload={onReload}
                />
              ) : (
                <div className={styles.startPage}>
                  <div className={styles.agentName}>{locale.agentName}</div>
                  <ChatInput
                    isRequesting={isRequesting || isPreparingModel}
                    deepThink={deepThink}
                    setDeepThink={setDeepThink}
                    onSubmit={handleSubmit}
                    onCancel={abort}
                    curConversation={curConversation}
                    modelOptions={modelOptions}
                    selectedModelId={selectedModelId}
                    onModelChange={setSelectedModelId}
                    modelLoading={modelLoading}
                    modelDisabled={isRequesting || isPreparingModel || modelOptions.length === 0}
                  />
                </div>
              )}

              {messages?.length !== 0 && (
                <ChatInput
                  isRequesting={isRequesting || isPreparingModel}
                  deepThink={deepThink}
                  setDeepThink={setDeepThink}
                  onSubmit={handleSubmit}
                  onCancel={abort}
                  curConversation={curConversation}
                  modelOptions={modelOptions}
                  selectedModelId={selectedModelId}
                  onModelChange={setSelectedModelId}
                  modelLoading={modelLoading}
                  modelDisabled={isRequesting || isPreparingModel || modelOptions.length === 0}
                />
              )}
            </div>
          </div>
        </div>
      </ChatContext.Provider>
    </XProvider>
  );
}

export default Chat;
