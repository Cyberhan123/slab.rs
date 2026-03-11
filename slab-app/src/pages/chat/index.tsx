import { XProvider } from '@ant-design/x';
import { useEffect, useMemo, useState } from 'react';
import { toast } from 'sonner';
import api from '@/lib/api';
import '@ant-design/x-markdown/themes/light.css';
import '@ant-design/x-markdown/themes/dark.css';
import { useMarkdownTheme } from './hooks/use-markdowm-theme';
import locale from './local';
import { ChatContext, DEFAULT_CONVERSATIONS_ITEMS, DEFAULT_CONVERSATION_KEY } from './chat-context';
import { useStyle } from './hooks/use-style';
import { ChatSidebar } from './components/chat-sidebar';
import { ChatMessageList } from './components/chat-message-list';
import { ChatInput } from './components/chat-input';
import { useChat } from './hooks/use-chat';

const LLAMA_BACKEND_ID = 'ggml.llama';
const MODEL_DOWNLOAD_POLL_INTERVAL_MS = 2_000;
const MODEL_DOWNLOAD_TIMEOUT_MS = 30 * 60 * 1_000;

type ModelOption = {
  id: string;
  label: string;
  downloaded: boolean;
  pending: boolean;
};

function Chat() {
  const [className] = useMarkdownTheme();
  const styles = useStyle();
  const [deepThink, setDeepThink] = useState<boolean>(true);
  const [curConversation, setCurConversation] = useState<string>(
    DEFAULT_CONVERSATIONS_ITEMS[0]?.key ?? DEFAULT_CONVERSATION_KEY,
  );

  const [selectedModelId, setSelectedModelId] = useState('');
  const [loadedModelId, setLoadedModelId] = useState<string | null>(null);

  const {
    data: catalogModels,
    isLoading: catalogModelsLoading,
    refetch: refetchCatalogModels,
  } = api.useQuery('get', '/v1/models');

  const downloadModelMutation = api.useMutation('post', '/v1/models/download');
  const loadModelMutation = api.useMutation('post', '/v1/models/load');
  const switchModelMutation = api.useMutation('post', '/v1/models/switch');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');

  const llamaModels = useMemo(
    () => (catalogModels ?? []).filter((model) => model.backend_ids.includes(LLAMA_BACKEND_ID)),
    [catalogModels]
  );

  const pendingTaskIdOf = (model: unknown): string | null => {
    if (typeof model !== 'object' || model === null) return null;
    const pendingTaskId = (model as { pending_task_id?: string | null }).pending_task_id;
    if (typeof pendingTaskId !== 'string') return null;
    const trimmed = pendingTaskId.trim();
    return trimmed.length > 0 ? trimmed : null;
  };

  const modelOptions = useMemo<ModelOption[]>(
    () =>
      llamaModels.map((model) => ({
        id: model.id,
        label: model.display_name,
        downloaded: Boolean(model.local_path),
        pending: Boolean(pendingTaskIdOf(model)),
      })),
    [llamaModels]
  );

  useEffect(() => {
    if (llamaModels.length === 0) {
      setSelectedModelId('');
      return;
    }

    const exists = llamaModels.some((model) => model.id === selectedModelId);
    if (!selectedModelId || !exists) {
      setSelectedModelId(llamaModels[0].id);
    }
  }, [llamaModels, selectedModelId]);

  const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

  const extractTaskId = (payload: unknown): string | null => {
    if (typeof payload !== 'object' || payload === null) return null;
    const taskId = (payload as { task_id?: unknown }).task_id;
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
    const models = refreshed.data ?? [];
    return models.find((model) => model.id === modelId);
  };

  const ensureDownloadedModelPath = async (
    modelId: string,
    forceDownload = false
  ): Promise<{ modelPath: string; downloadedNow: boolean }> => {
    let model = llamaModels.find((item) => item.id === modelId);
    if (!model) {
      model = await refreshCatalogAndFindModel(modelId);
    }

    if (!model) {
      throw new Error('Selected model does not exist in catalog');
    }

    if (!model.backend_ids.includes(LLAMA_BACKEND_ID)) {
      throw new Error(`Selected model does not support ${LLAMA_BACKEND_ID}`);
    }

    if (model.local_path && !forceDownload) {
      return { modelPath: model.local_path, downloadedNow: false };
    }

    let taskId = pendingTaskIdOf(model);
    if (!taskId) {
      const downloadResponse = await downloadModelMutation.mutateAsync({
        body: {
          backend_id: LLAMA_BACKEND_ID,
          model_id: modelId,
        },
      });
      taskId = extractTaskId(downloadResponse);
    }

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

    const selected = llamaModels.find((item) => item.id === selectedModelId);
    const { modelPath, downloadedNow } = await ensureDownloadedModelPath(selectedModelId);

    if (downloadedNow) {
      toast.success(`Downloaded ${selected?.display_name ?? selectedModelId}`);
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
        toast.success(`Downloaded ${selected?.display_name ?? selectedModelId}`);
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
  } = useChat(curConversation, selectedModelId || 'slab-llama', ensureChatModelReady);

  const isPreparingModel =
    loadModelMutation.isPending ||
    switchModelMutation.isPending ||
    downloadModelMutation.isPending;

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
                    modelLoading={catalogModelsLoading}
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
                  modelLoading={catalogModelsLoading}
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
