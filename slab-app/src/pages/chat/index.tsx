import { XProvider } from "@ant-design/x"
import { useXConversations, type ConversationData } from "@ant-design/x-sdk"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { useNavigate } from "react-router-dom"
import { toast } from "sonner"

import "@ant-design/x-markdown/themes/dark.css"
import "@ant-design/x-markdown/themes/light.css"

import { Badge } from "@/components/ui/badge"
import { ScrollArea } from "@/components/ui/scroll-area"
import { ChatComposer } from "@/pages/chat/components/chat-composer"
import { ChatMessageBubble } from "@/pages/chat/components/chat-message-bubble"
import { ChatSessionSheet } from "@/pages/chat/components/chat-session-sheet"
import { ChatSessionSummaryCard } from "@/pages/chat/components/chat-session-summary-card"
import { ChatWelcome } from "@/pages/chat/components/chat-welcome"
import api from "@/lib/api"
import { toCatalogModelList } from "@/lib/api/models"
import { PAGE_HEADER_META } from "@/layouts/header-meta"
import { usePageHeader } from "@/hooks/use-global-header-meta"

import {
  API_BASE_URL,
  ChatContext,
  DEFAULT_CONVERSATIONS_ITEMS,
  DEFAULT_CONVERSATION_KEY,
} from "./chat-context"
import { useChat } from "./hooks/use-chat"
import { useMarkdownTheme } from "./hooks/use-markdowm-theme"
import locale from "./local"

const LLAMA_BACKEND_ID = "ggml.llama"
const MODEL_DOWNLOAD_POLL_INTERVAL_MS = 2_000
const MODEL_DOWNLOAD_TIMEOUT_MS = 30 * 60 * 1_000

type ModelOptionSource = "local" | "cloud"

type ModelOption = {
  id: string
  label: string
  downloaded: boolean
  pending: boolean
  source: ModelOptionSource
}

type ChatModelApiItem = {
  id: string
  display_name: string
  source: ModelOptionSource
  downloaded: boolean
  pending: boolean
  provider_name?: string | null
}

type ConversationItem = ConversationData & {
  label?: string
  group?: string
}

type ChatMessageRecord = {
  id: string | number
  status?: string
  message: {
    role: "assistant" | "user"
    content?: string | null
    extraInfo?: unknown
  }
}

function isChatModelApiItem(value: unknown): value is ChatModelApiItem {
  if (typeof value !== "object" || value === null) return false
  const obj = value as Record<string, unknown>

  return (
    typeof obj.id === "string" &&
    typeof obj.display_name === "string" &&
    (obj.source === "local" || obj.source === "cloud") &&
    typeof obj.downloaded === "boolean" &&
    typeof obj.pending === "boolean"
  )
}

function createConversationLabel(value: string) {
  const trimmed = value.trim()

  if (!trimmed) {
    return "New chat"
  }

  return trimmed.length > 42 ? `${trimmed.slice(0, 42)}...` : trimmed
}

function Chat() {
  const navigate = useNavigate()
  const [markdownThemeClassName] = useMarkdownTheme()
  const [deepThink, setDeepThink] = useState(true)
  const [draft, setDraft] = useState("")
  const [isSessionSheetOpen, setIsSessionSheetOpen] = useState(false)
  const [curConversation, setCurConversation] = useState<string>(
    DEFAULT_CONVERSATIONS_ITEMS[0]?.key ?? DEFAULT_CONVERSATION_KEY
  )

  const [selectedModelId, setSelectedModelId] = useState("")
  const [loadedModelId, setLoadedModelId] = useState<string | null>(null)
  const [cloudModelOptions, setCloudModelOptions] = useState<ModelOption[]>([])
  const [cloudModelsLoading, setCloudModelsLoading] = useState(false)
  const bottomRef = useRef<HTMLDivElement | null>(null)

  const { conversations, addConversation, setConversations } = useXConversations({
    defaultConversations: DEFAULT_CONVERSATIONS_ITEMS,
  })
  const conversationList = conversations as ConversationItem[]

  const {
    data: catalogModels,
    isLoading: catalogModelsLoading,
    refetch: refetchCatalogModels,
  } = api.useQuery("get", "/v1/models")

  const downloadModelMutation = api.useMutation("post", "/v1/models/download")
  const loadModelMutation = api.useMutation("post", "/v1/models/load")
  const switchModelMutation = api.useMutation("post", "/v1/models/switch")
  const getTaskMutation = api.useMutation("get", "/v1/tasks/{id}")

  const loadCloudModels = useCallback(async () => {
    setCloudModelsLoading(true)

    try {
      const response = await fetch(`${API_BASE_URL}/v1/chat/models`, {
        method: "GET",
      })

      if (!response.ok) {
        const detail = await response.text()
        throw new Error(`HTTP ${response.status}: ${detail || "failed to load models"}`)
      }

      const payload: unknown = await response.json()
      if (!Array.isArray(payload)) {
        throw new Error("Invalid chat model payload")
      }

      const cloudOnly = payload
        .filter((item): item is ChatModelApiItem => isChatModelApiItem(item))
        .filter((item) => item.source === "cloud")
        .map<ModelOption>((item) => ({
          id: item.id,
          label: item.provider_name
            ? `${item.provider_name} / ${item.display_name}`
            : item.display_name,
          downloaded: item.downloaded,
          pending: item.pending,
          source: "cloud",
        }))

      setCloudModelOptions(cloudOnly)
    } catch (error: any) {
      setCloudModelOptions([])
      toast.error("Failed to load cloud model options", {
        description: error?.message || "Unknown error",
      })
    } finally {
      setCloudModelsLoading(false)
    }
  }, [])

  useEffect(() => {
    void loadCloudModels()
  }, [loadCloudModels])

  const parsedCatalogModels = useMemo(
    () => toCatalogModelList(catalogModels),
    [catalogModels]
  )

  const llamaModels = useMemo(
    () => parsedCatalogModels.filter((model) => model.backend_id === LLAMA_BACKEND_ID),
    [parsedCatalogModels]
  )

  const localModelOptions = useMemo<ModelOption[]>(
    () =>
      llamaModels.map((model) => ({
        id: model.id,
        label: model.display_name,
        downloaded: Boolean(model.local_path),
        pending: model.pending,
        source: "local",
      })),
    [llamaModels]
  )

  const modelOptions = useMemo<ModelOption[]>(
    () => [...localModelOptions, ...cloudModelOptions],
    [localModelOptions, cloudModelOptions]
  )

  useEffect(() => {
    if (modelOptions.length === 0) {
      setSelectedModelId("")
      return
    }

    const exists = modelOptions.some((model) => model.id === selectedModelId)
    if (!selectedModelId || !exists) {
      setSelectedModelId(modelOptions[0].id)
    }
  }, [modelOptions, selectedModelId])

  useEffect(() => {
    if (!conversationList.some((item) => item.key === curConversation)) {
      setCurConversation(conversationList[0]?.key ?? DEFAULT_CONVERSATION_KEY)
    }
  }, [conversationList, curConversation])

  const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms))

  const extractTaskId = (payload: unknown): string | null => {
    if (typeof payload !== "object" || payload === null) return null

    const taskId =
      (payload as { operation_id?: unknown }).operation_id ??
      (payload as { task_id?: unknown }).task_id

    if (typeof taskId !== "string") return null

    const trimmed = taskId.trim()
    return trimmed.length > 0 ? trimmed : null
  }

  const waitForTaskToFinish = async (taskId: string) => {
    const deadline = Date.now() + MODEL_DOWNLOAD_TIMEOUT_MS

    while (Date.now() < deadline) {
      const task = (await getTaskMutation.mutateAsync({
        params: { path: { id: taskId } },
      })) as { status: string; error_msg?: string | null }

      if (task.status === "succeeded") {
        return
      }

      if (
        task.status === "failed" ||
        task.status === "cancelled" ||
        task.status === "interrupted"
      ) {
        throw new Error(task.error_msg ?? `Task ${taskId} ended with status: ${task.status}`)
      }

      await sleep(MODEL_DOWNLOAD_POLL_INTERVAL_MS)
    }

    throw new Error("Model download timed out")
  }

  const refreshCatalogAndFindModel = async (modelId: string) => {
    const refreshed = await refetchCatalogModels()
    const models = toCatalogModelList(refreshed.data)
    return models.find((model) => model.id === modelId)
  }

  const ensureDownloadedModelPath = async (
    modelId: string,
    forceDownload = false
  ): Promise<{ modelPath: string; downloadedNow: boolean }> => {
    let model = llamaModels.find((item) => item.id === modelId)
    if (!model) {
      model = await refreshCatalogAndFindModel(modelId)
    }

    if (!model) {
      throw new Error("Selected model does not exist in catalog")
    }

    if (model.backend_id !== LLAMA_BACKEND_ID) {
      throw new Error(`Selected model does not support ${LLAMA_BACKEND_ID}`)
    }

    if (model.local_path && !forceDownload) {
      return { modelPath: model.local_path, downloadedNow: false }
    }

    const downloadResponse = await downloadModelMutation.mutateAsync({
      body: {
        model_id: modelId,
      },
    })
    const taskId = extractTaskId(downloadResponse)

    if (!taskId) {
      throw new Error("Failed to start model download task")
    }

    await waitForTaskToFinish(taskId)

    const refreshedModel = await refreshCatalogAndFindModel(modelId)
    if (!refreshedModel?.local_path) {
      throw new Error("Model download completed, but local_path is empty")
    }

    return { modelPath: refreshedModel.local_path, downloadedNow: true }
  }

  const loadOrSwitchSelectedModel = async (modelPath: string) => {
    const shouldSwitch = Boolean(loadedModelId && loadedModelId !== selectedModelId)

    if (shouldSwitch) {
      await switchModelMutation.mutateAsync({
        body: {
          backend_id: LLAMA_BACKEND_ID,
          model_path: modelPath,
        },
      })
      return
    }

    await loadModelMutation.mutateAsync({
      body: {
        backend_id: LLAMA_BACKEND_ID,
        model_path: modelPath,
      },
    })
  }

  const prepareSelectedModel = async () => {
    if (!selectedModelId) {
      throw new Error("Please select a chat model first.")
    }

    if (loadedModelId === selectedModelId) {
      return
    }

    const selectedOption = modelOptions.find((item) => item.id === selectedModelId)
    if (!selectedOption) {
      throw new Error("Selected model is not available")
    }

    if (selectedOption.source === "cloud") {
      setLoadedModelId(selectedModelId)
      return
    }

    const selectedLocal = llamaModels.find((item) => item.id === selectedModelId)
    const { modelPath, downloadedNow } = await ensureDownloadedModelPath(selectedModelId)

    if (downloadedNow) {
      toast.success(`Downloaded ${selectedLocal?.display_name ?? selectedModelId}`)
    }

    try {
      await loadOrSwitchSelectedModel(modelPath)
    } catch (firstLoadError) {
      if (downloadedNow) {
        throw firstLoadError
      }

      toast.message("Model load failed, re-downloading and retrying once...")

      const retry = await ensureDownloadedModelPath(selectedModelId, true)
      if (retry.downloadedNow) {
        toast.success(`Downloaded ${selectedLocal?.display_name ?? selectedModelId}`)
      }

      await loadOrSwitchSelectedModel(retry.modelPath)
    }

    setLoadedModelId(selectedModelId)
  }

  const ensureChatModelReady = async () => {
    try {
      await prepareSelectedModel()
    } catch (err: any) {
      toast.error("Failed to prepare chat model.", {
        description: err?.message || err?.error || "Unknown error",
      })
      throw err
    }
  }

  const {
    messages,
    isRequesting,
    abort,
    onReload,
    activeConversation,
    handleSubmit,
  } = useChat(curConversation, selectedModelId || "slab-llama", deepThink, ensureChatModelReady)

  const modelLoading = catalogModelsLoading || cloudModelsLoading
  const isPreparingModel =
    loadModelMutation.isPending ||
    switchModelMutation.isPending ||
    downloadModelMutation.isPending

  const safeMessages = (messages ?? []) as ChatMessageRecord[]
  const currentConversationRecord = conversationList.find((item) => item.key === curConversation)
  const selectedModelLabel =
    modelOptions.find((item) => item.id === selectedModelId)?.label ?? "Select model"
  const latestUserPrompt =
    safeMessages
      .slice()
      .reverse()
      .find((item) => item.message.role === "user")
      ?.message.content?.trim() ?? ""

  usePageHeader({
    ...PAGE_HEADER_META.chat,
    subtitle: `Talk with AI models in one workspace - ${selectedModelLabel}`,
  })

  useEffect(() => {
    bottomRef.current?.scrollIntoView({
      behavior: safeMessages.length > 0 ? "smooth" : "auto",
      block: "end",
    })
  }, [safeMessages, isRequesting])

  const sortedConversations = useMemo(() => {
    return [...conversationList].sort((a, b) => {
      if (a.key === curConversation) return -1
      if (b.key === curConversation) return 1
      return String(b.key).localeCompare(String(a.key))
    })
  }, [conversationList, curConversation])

  const setConversationLabelIfNeeded = useCallback(
    (conversationKey: string, prompt: string) => {
      const nextList = conversationList.map((item) => {
        const label = item.label ?? "New chat"

        if (item.key !== conversationKey) {
          return item
        }

        if (label !== "New chat" && label !== "New Conversation") {
          return item
        }

        return {
          ...item,
          label: createConversationLabel(prompt),
        }
      })

      setConversations(nextList)
    },
    [conversationList, setConversations]
  )

  const handleCreateConversation = useCallback(() => {
    if (safeMessages.length === 0) {
      toast.info("The current session is already empty.")
      return
    }

    const now = Date.now().toString()
    addConversation(
      {
        key: now,
        label: "New chat",
        group: "Workspace",
      },
      "prepend"
    )
    setCurConversation(now)
    setDraft("")
    setIsSessionSheetOpen(true)
  }, [addConversation, safeMessages.length])

  const handleDeleteConversation = useCallback(
    (conversationKey: string) => {
      const nextList = conversationList.filter((item) => item.key !== conversationKey)

      if (nextList.length === 0) {
        setConversations(DEFAULT_CONVERSATIONS_ITEMS)
        setCurConversation(DEFAULT_CONVERSATION_KEY)
        return
      }

      setConversations(nextList)
      if (conversationKey === curConversation) {
        setCurConversation(nextList[0]?.key ?? DEFAULT_CONVERSATION_KEY)
      }
    },
    [conversationList, curConversation, setConversations]
  )

  const handleGenerateImage = useCallback(() => {
    const prompt = draft.trim() || latestUserPrompt
    const search = prompt ? `?prompt=${encodeURIComponent(prompt)}` : ""

    navigate(
      {
        pathname: "/image",
        search,
      },
      {
        state: prompt ? { prompt } : undefined,
      }
    )
  }, [draft, latestUserPrompt, navigate])

  const submitChatMessage = useCallback(
    async (value: string) => {
      setConversationLabelIfNeeded(curConversation, value)
      setDraft("")
      await handleSubmit(value)
    },
    [curConversation, handleSubmit, setConversationLabelIfNeeded]
  )

  return (
    <XProvider locale={locale}>
      <ChatContext.Provider value={{ onReload }}>
        <div className="relative flex min-h-0 flex-1 flex-col">
          <div className="flex flex-wrap items-start justify-between gap-4 pb-4">
            <div className="max-w-2xl space-y-3">
              <Badge variant="chip">Single-column chat</Badge>
              <div className="space-y-2">
                <h1 className="text-3xl font-semibold tracking-tight md:text-4xl">
                  Keep the thread focused and move fast.
                </h1>
                <p className="text-sm leading-7 text-muted-foreground md:text-base">
                  Sessions live in a sheet, the message rail stays centered, and composer tools stay
                  close to the prompt instead of scattered across the page.
                </p>
              </div>
            </div>

            <ChatSessionSummaryCard
              title={currentConversationRecord?.label ?? "New chat"}
              messageCount={safeMessages.length}
              modelLabel={selectedModelLabel}
              deepThink={deepThink}
              onManageSessions={() => setIsSessionSheetOpen(true)}
              onNewSession={handleCreateConversation}
            />
          </div>

          <ScrollArea className="min-h-0 flex-1">
            <div className="mx-auto flex w-full max-w-3xl flex-col gap-6 px-1 pb-56 pt-2">
              {safeMessages.length === 0 ? (
                <ChatWelcome
                  agentName={locale.agentName}
                  onUsePrompt={setDraft}
                  onGenerateImage={handleGenerateImage}
                  onNewSession={handleCreateConversation}
                />
              ) : (
                safeMessages.map((item) => (
                  <ChatMessageBubble
                    key={item.id}
                    item={item}
                    markdownClassName={markdownThemeClassName}
                    onRetry={(id) =>
                      onReload(id, {
                        userAction: "retry",
                      })
                    }
                  />
                ))
              )}
              <div ref={bottomRef} />
            </div>
          </ScrollArea>

          <div className="pointer-events-none absolute inset-x-0 bottom-0">
            <div className="h-24 bg-gradient-to-t from-[var(--surface-1)] via-[color:color-mix(in_oklab,var(--surface-1)_92%,transparent)] to-transparent" />
            <div className="pointer-events-auto mx-auto -mt-2 w-full max-w-3xl px-1 pb-4">
              <ChatComposer
                value={draft}
                onValueChange={setDraft}
                onSubmit={submitChatMessage}
                onCancel={abort}
                isRequesting={isRequesting || isPreparingModel}
                deepThink={deepThink}
                setDeepThink={setDeepThink}
                modelOptions={modelOptions}
                selectedModelId={selectedModelId}
                onModelChange={setSelectedModelId}
                modelLoading={modelLoading}
                modelDisabled={
                  isRequesting || isPreparingModel || modelOptions.length === 0
                }
                onGenerateImage={handleGenerateImage}
              />
            </div>
          </div>

          <ChatSessionSheet
            open={isSessionSheetOpen}
            onOpenChange={setIsSessionSheetOpen}
            conversations={sortedConversations}
            currentConversation={curConversation}
            activeConversation={activeConversation}
            onSelect={(key) => {
              setCurConversation(key)
              setIsSessionSheetOpen(false)
            }}
            onCreate={handleCreateConversation}
            onDelete={handleDeleteConversation}
          />
        </div>
      </ChatContext.Provider>
    </XProvider>
  )
}

export default Chat
