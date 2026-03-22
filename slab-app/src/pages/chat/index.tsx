import { XProvider } from "@ant-design/x"
import { useXConversations, type ConversationData } from "@ant-design/x-sdk"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { useNavigate } from "react-router-dom"
import { toast } from "sonner"

import "@ant-design/x-markdown/themes/dark.css"
import "@ant-design/x-markdown/themes/light.css"

import { ScrollArea } from "@/components/ui/scroll-area"
import { ChatComposer } from "@/pages/chat/components/chat-composer"
import { ChatMessageBubble } from "@/pages/chat/components/chat-message-bubble"
import { ChatSessionSheet } from "@/pages/chat/components/chat-session-sheet"
import {
  ChatSessionSummaryCard,
  type ChatSessionSummaryItem,
} from "@/pages/chat/components/chat-session-summary-card"
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
  contextWindow?: number | null
}

type ChatModelApiItem = {
  id: string
  display_name: string
  source: ModelOptionSource
  downloaded: boolean
  pending: boolean
  provider_name?: string | null
  context_window?: number | null
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
    typeof obj.pending === "boolean" &&
    (obj.context_window === undefined ||
      obj.context_window === null ||
      typeof obj.context_window === "number")
  )
}

function createConversationLabel(value: string) {
  const trimmed = value.trim()

  if (!trimmed) {
    return "New chat"
  }

  return trimmed.length > 42 ? `${trimmed.slice(0, 42)}...` : trimmed
}

function getGreeting(date: Date) {
  const hour = date.getHours()

  if (hour < 12) {
    return "Good morning"
  }

  if (hour < 18) {
    return "Good afternoon"
  }

  return "Good evening"
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
          contextWindow: item.context_window ?? null,
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
        contextWindow: model.spec.context_window ?? null,
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
  const selectedModel = modelOptions.find((item) => item.id === selectedModelId)
  const selectedModelLabel = selectedModel?.label ?? "Select model"
  const selectedModelStatusLabel = useMemo(() => {
    if (modelLoading) {
      return "Loading models"
    }

    if (!selectedModel) {
      return "Select model"
    }

    const parts = [selectedModel.label]

    if (selectedModel.contextWindow && selectedModel.contextWindow > 0) {
      parts.push(
        `${new Intl.NumberFormat("en-US").format(selectedModel.contextWindow)} Context`
      )
    } else if (selectedModel.pending) {
      parts.push("Downloading")
    } else if (selectedModel.source === "local" && !selectedModel.downloaded) {
      parts.push("Needs download")
    } else if (isPreparingModel) {
      parts.push("Preparing")
    } else if (selectedModel.source === "cloud") {
      parts.push("Cloud model")
    }

    return parts.join(" / ")
  }, [isPreparingModel, modelLoading, selectedModel])
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
  const greeting = useMemo(() => getGreeting(new Date()), [])
  const sessionSummaryItems = useMemo<ChatSessionSummaryItem[]>(() => {
    return sortedConversations.slice(0, 2).map<ChatSessionSummaryItem>((conversation, index) => ({
      key: conversation.key,
      label: conversation.label ?? (index === 0 ? "Current session" : "Next session"),
      hint:
        conversation.key === curConversation
          ? `${safeMessages.length} ${safeMessages.length === 1 ? "message" : "messages"}`
          : conversation.group ?? "Workspace",
      tone: index === 0 ? "warm" : "mint",
    }))
  }, [curConversation, safeMessages.length, sortedConversations])

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
        <div className="relative flex min-h-0 flex-1 flex-col bg-[var(--shell-card)]">
          <div className="pointer-events-none absolute right-4 top-4 hidden xl:block">
            <div className="pointer-events-auto">
              <ChatSessionSummaryCard
                items={sessionSummaryItems}
                onManageSessions={() => setIsSessionSheetOpen(true)}
                onNewSession={handleCreateConversation}
              />
            </div>
          </div>

          <div className="mx-auto w-full max-w-[768px] px-6 pb-6 pt-12 md:px-8 xl:px-0">
            <div className="space-y-2">
              <h1 className="text-[clamp(2.75rem,6vw,4rem)] font-semibold tracking-[-0.055em] text-foreground">
                {greeting}
              </h1>
              <p className="text-lg leading-7 text-muted-foreground/80">
                How can I assist your creative workflow today?
              </p>
            </div>
          </div>

          <div className="mx-auto block w-full max-w-[768px] px-6 pb-6 md:px-8 xl:hidden xl:px-0">
            <ChatSessionSummaryCard
              items={sessionSummaryItems}
              onManageSessions={() => setIsSessionSheetOpen(true)}
              onNewSession={handleCreateConversation}
            />
          </div>

          <ScrollArea className="min-h-0 flex-1">
            <div className="mx-auto flex w-full max-w-[682px] flex-col gap-8 px-6 pb-56 pt-2 md:px-8 xl:px-0">
              {safeMessages.length === 0 ? (
                <div className="flex min-h-[260px] items-center justify-center rounded-[32px] border border-dashed border-border/60 bg-[linear-gradient(180deg,color-mix(in_oklab,var(--app-canvas)_90%,transparent)_0%,color-mix(in_oklab,var(--app-canvas)_50%,transparent)_100%)] px-8 text-center">
                  <div className="max-w-md space-y-3">
                    <p className="text-base font-medium text-foreground">
                      Start a new thread and keep the stage focused.
                    </p>
                    <p className="text-sm leading-6 text-muted-foreground">
                      Ask for debugging help, refine a draft, or pass the current idea into image
                      generation when it needs a visual direction.
                    </p>
                  </div>
                </div>
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
            <div className="absolute inset-x-0 bottom-0 h-28 bg-gradient-to-t from-[var(--app-canvas)] via-[color-mix(in_oklab,var(--app-canvas)_92%,transparent)] to-transparent" />
            <div className="pointer-events-auto relative mx-auto w-full max-w-[768px] px-6 pb-6 pt-[4.25rem] md:px-8 xl:px-0">
              <ChatComposer
                value={draft}
                onValueChange={setDraft}
                onSubmit={submitChatMessage}
                onCancel={abort}
                isRequesting={isRequesting || isPreparingModel}
                deepThink={deepThink}
                setDeepThink={setDeepThink}
                onGenerateImage={handleGenerateImage}
                statusLabel={selectedModelStatusLabel}
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
