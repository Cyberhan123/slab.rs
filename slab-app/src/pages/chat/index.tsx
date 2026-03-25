import { XProvider } from "@ant-design/x"
import { useXConversations, type ConversationData } from "@ant-design/x-sdk"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { useNavigate } from "react-router-dom"
import { toast } from "sonner"

import "@ant-design/x-markdown/themes/dark.css"
import "@ant-design/x-markdown/themes/light.css"

import { ScrollArea } from "@/components/ui/scroll-area"
import type { components } from "@/lib/api/v1.d.ts"
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
import { usePageHeader, usePageHeaderModelPicker } from "@/hooks/use-global-header-meta"

import {
  ChatContext,
  clearConversationCache,
  getChatMessageTextContent,
  type ChatMessageRecord,
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

type ConversationItem = ConversationData & {
  label?: string
  group?: string
}

type SessionRecord = components["schemas"]["SessionResponse"]
type SessionLabelMap = Record<string, string>

const CHAT_CURRENT_SESSION_STORAGE_KEY = "slab.chat.currentSessionId"
const CHAT_SESSION_LABELS_STORAGE_KEY = "slab.chat.sessionLabels"

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

function getErrorDescription(error: unknown) {
  if (error instanceof Error && error.message.trim()) {
    return error.message
  }

  if (typeof error === "object" && error !== null) {
    const message = (error as { error?: unknown; message?: unknown }).message
    if (typeof message === "string" && message.trim()) {
      return message
    }

    const rawError = (error as { error?: unknown }).error
    if (typeof rawError === "string" && rawError.trim()) {
      return rawError
    }
  }

  return "Unknown error"
}

function readStoredCurrentSessionId() {
  if (typeof window === "undefined") {
    return ""
  }

  try {
    return window.localStorage.getItem(CHAT_CURRENT_SESSION_STORAGE_KEY)?.trim() ?? ""
  } catch {
    return ""
  }
}

function writeStoredCurrentSessionId(sessionId: string) {
  if (typeof window === "undefined") {
    return
  }

  try {
    const trimmed = sessionId.trim()
    if (!trimmed) {
      window.localStorage.removeItem(CHAT_CURRENT_SESSION_STORAGE_KEY)
      return
    }

    window.localStorage.setItem(CHAT_CURRENT_SESSION_STORAGE_KEY, trimmed)
  } catch {
    // Ignore storage failures and keep the session in memory.
  }
}

function readStoredSessionLabels(): SessionLabelMap {
  if (typeof window === "undefined") {
    return {}
  }

  try {
    const raw = window.localStorage.getItem(CHAT_SESSION_LABELS_STORAGE_KEY)
    if (!raw) {
      return {}
    }

    const parsed = JSON.parse(raw) as unknown
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      return {}
    }

    return Object.fromEntries(
      Object.entries(parsed).filter(
        ([key, value]) => typeof key === "string" && typeof value === "string" && value.trim()
      )
    )
  } catch {
    return {}
  }
}

function writeStoredSessionLabels(labels: SessionLabelMap) {
  if (typeof window === "undefined") {
    return
  }

  try {
    window.localStorage.setItem(CHAT_SESSION_LABELS_STORAGE_KEY, JSON.stringify(labels))
  } catch {
    // Ignore storage failures and keep the labels in memory.
  }
}

function getStoredSessionLabel(sessionId: string) {
  const value = readStoredSessionLabels()[sessionId]?.trim()
  return value ? value : null
}

function setStoredSessionLabel(sessionId: string, label: string) {
  const trimmedSessionId = sessionId.trim()
  const trimmedLabel = label.trim()

  if (!trimmedSessionId || !trimmedLabel) {
    return
  }

  writeStoredSessionLabels({
    ...readStoredSessionLabels(),
    [trimmedSessionId]: trimmedLabel,
  })
}

function removeStoredSessionLabel(sessionId: string) {
  const trimmedSessionId = sessionId.trim()
  if (!trimmedSessionId) {
    return
  }

  const nextLabels = readStoredSessionLabels()
  delete nextLabels[trimmedSessionId]
  writeStoredSessionLabels(nextLabels)
}

function toConversationItem(session: SessionRecord): ConversationItem {
  const storedLabel = getStoredSessionLabel(session.id)
  const backendLabel = session.name.trim()

  return {
    key: session.id,
    label: storedLabel ?? (backendLabel || "New chat"),
    group: "Workspace",
  }
}

function Chat() {
  const navigate = useNavigate()
  const [markdownThemeClassName] = useMarkdownTheme()
  const [deepThink, setDeepThink] = useState(true)
  const [draft, setDraft] = useState("")
  const [isSessionSheetOpen, setIsSessionSheetOpen] = useState(false)
  const [curConversation, setCurConversation] = useState<string>(() => readStoredCurrentSessionId())

  const [selectedModelId, setSelectedModelId] = useState("")
  const [loadedModelId, setLoadedModelId] = useState<string | null>(null)
  const bottomRef = useRef<HTMLDivElement | null>(null)
  const hasBootstrappedSessions = useRef(false)

  const { conversations, addConversation, setConversations } = useXConversations({
    defaultConversations: [],
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
  const { data: sessionData, isLoading: sessionsLoading, refetch: refetchSessions } = api.useQuery(
    "get",
    "/v1/sessions"
  )
  const createSessionMutation = api.useMutation("post", "/v1/sessions")
  // Generated types currently drop the DELETE path param for this endpoint.
  const deleteSessionMutation = api.useMutation("delete", "/v1/sessions/{id}") as unknown as {
    isPending: boolean
    mutateAsync: (options: { params: { path: { id: string } } }) => Promise<unknown>
  }

  const parsedCatalogModels = useMemo(
    () => toCatalogModelList(catalogModels),
    [catalogModels]
  )
  const sessionRecords = useMemo<SessionRecord[]>(
    () => (Array.isArray(sessionData) ? sessionData : []),
    [sessionData]
  )

  const chatCatalogModels = useMemo(
    () =>
      parsedCatalogModels.filter(
        (model) =>
          model.backend_id === LLAMA_BACKEND_ID || Boolean(model.spec.remote_model_id)
      ),
    [parsedCatalogModels]
  )

  const llamaModels = useMemo(
    () => chatCatalogModels.filter((model) => model.backend_id === LLAMA_BACKEND_ID),
    [chatCatalogModels]
  )

  const modelOptions = useMemo<ModelOption[]>(
    () =>
      chatCatalogModels.map((model) => ({
        id: model.id,
        label:
          model.backend_id === LLAMA_BACKEND_ID
            ? model.display_name
            : `${formatProviderLabel(model.provider)} / ${model.display_name}`,
        downloaded: model.backend_id === LLAMA_BACKEND_ID ? Boolean(model.local_path) : true,
        pending: model.pending,
        source: model.backend_id === LLAMA_BACKEND_ID ? "local" : "cloud",
        contextWindow: model.spec.context_window ?? null,
      })),
    [chatCatalogModels]
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
    setConversations(sessionRecords.map(toConversationItem))
  }, [sessionRecords, setConversations])

  const createEmptySession = useCallback(
    async (options?: { openSheet?: boolean; quiet?: boolean; select?: boolean }) => {
      try {
        const session = await createSessionMutation.mutateAsync({
          body: {},
        })

        addConversation(toConversationItem(session), "prepend")
        if (options?.select ?? true) {
          setCurConversation(session.id)
        }
        if (options?.openSheet) {
          setIsSessionSheetOpen(true)
        }

        void refetchSessions()
        return session
      } catch (error) {
        if (!options?.quiet) {
          toast.error("Failed to create chat session.", {
            description: getErrorDescription(error),
          })
        }
        return null
      }
    },
    [addConversation, createSessionMutation, refetchSessions]
  )

  useEffect(() => {
    if (sessionsLoading) {
      return
    }

    if (sessionRecords.length > 0) {
      hasBootstrappedSessions.current = true
      return
    }

    if (hasBootstrappedSessions.current) {
      return
    }

    hasBootstrappedSessions.current = true
    void createEmptySession({ quiet: true, select: true })
  }, [createEmptySession, sessionRecords.length, sessionsLoading])

  useEffect(() => {
    if (conversationList.length === 0) {
      if (curConversation) {
        setCurConversation("")
      }
      return
    }

    if (conversationList.some((item) => item.key === curConversation)) {
      return
    }

    const storedConversationId = readStoredCurrentSessionId()
    const nextConversationKey =
      conversationList.find((item) => item.key === storedConversationId)?.key ??
      conversationList[0]?.key ??
      ""

    if (nextConversationKey && nextConversationKey !== curConversation) {
      setCurConversation(nextConversationKey)
    }
  }, [conversationList, curConversation])

  useEffect(() => {
    writeStoredCurrentSessionId(curConversation)
  }, [curConversation])

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

  const loadOrSwitchSelectedModel = async (modelId: string) => {
    const shouldSwitch = Boolean(loadedModelId && loadedModelId !== selectedModelId)

    if (shouldSwitch) {
      await switchModelMutation.mutateAsync({
        body: {
          model_id: modelId,
        },
      })
      return
    }

    await loadModelMutation.mutateAsync({
      body: {
        model_id: modelId,
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
    const { downloadedNow } = await ensureDownloadedModelPath(selectedModelId)

    if (downloadedNow) {
      toast.success(`Downloaded ${selectedLocal?.display_name ?? selectedModelId}`)
    }

    try {
      await loadOrSwitchSelectedModel(selectedModelId)
    } catch (firstLoadError) {
      if (downloadedNow) {
        throw firstLoadError
      }

      toast.message("Model load failed, re-downloading and retrying once...")

      const retry = await ensureDownloadedModelPath(selectedModelId, true)
      if (retry.downloadedNow) {
        toast.success(`Downloaded ${selectedLocal?.display_name ?? selectedModelId}`)
      }

      await loadOrSwitchSelectedModel(selectedModelId)
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
    isHistoryLoading,
    abort,
    onReload,
    onContinue,
    activeConversation,
    handleSubmit,
  } = useChat(curConversation, selectedModelId || "slab-llama", deepThink, ensureChatModelReady)

  const modelLoading = catalogModelsLoading
  const isPreparingModel =
    loadModelMutation.isPending ||
    switchModelMutation.isPending ||
    downloadModelMutation.isPending
  const isSessionMutating = createSessionMutation.isPending || deleteSessionMutation.isPending
  const isSessionBusy = isRequesting || isPreparingModel || isHistoryLoading || isSessionMutating
  const isSessionBootstrapping =
    (sessionsLoading || createSessionMutation.isPending) && conversationList.length === 0

  const safeMessages: ChatMessageRecord[] = messages ?? []
  const latestMessage = safeMessages[safeMessages.length - 1]
  const latestContinuableMessageId =
    latestMessage?.message.role === "assistant" && latestMessage.status === "abort"
      ? (() => {
          const content = getChatMessageTextContent(latestMessage.message).trim()
          return content && content !== locale.requestAborted && content !== locale.noData
            ? latestMessage.id
            : undefined
        })()
      : undefined
  const selectedModel = modelOptions.find((item) => item.id === selectedModelId)
  const latestUserMessage = safeMessages
    .slice()
    .reverse()
    .find((item) => item.message.role === "user")
  const selectedModelStatusLabel = useMemo(() => {
    if (isSessionBootstrapping || !curConversation) {
      return "Preparing session"
    }

    if (isHistoryLoading) {
      return "Loading session history"
    }

    if (createSessionMutation.isPending) {
      return "Creating session"
    }

    if (deleteSessionMutation.isPending) {
      return "Deleting session"
    }

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
  }, [
    createSessionMutation.isPending,
    curConversation,
    deleteSessionMutation.isPending,
    isHistoryLoading,
    isPreparingModel,
    isSessionBootstrapping,
    modelLoading,
    selectedModel,
  ])
  const headerModelPicker = useMemo(
    () => ({
      value: selectedModelId,
      options: modelOptions.map((model) => ({
        id: model.id,
        label: model.label,
      })),
      onValueChange: setSelectedModelId,
      groupLabel: "Chat Models",
      placeholder: "Select model",
      loading: modelLoading,
      disabled:
        modelLoading ||
        isPreparingModel ||
        isRequesting ||
        modelOptions.length === 0,
      emptyLabel: "No chat models",
    }),
    [isPreparingModel, isRequesting, modelLoading, modelOptions, selectedModelId]
  )
  const latestUserPrompt = getChatMessageTextContent(latestUserMessage?.message).trim()

  usePageHeader(PAGE_HEADER_META.chat)
  usePageHeaderModelPicker(headerModelPicker)

  useEffect(() => {
    bottomRef.current?.scrollIntoView({
      behavior: safeMessages.length > 0 ? "smooth" : "auto",
      block: "end",
    })
  }, [safeMessages, isRequesting])

  const sortedConversations = useMemo(() => {
    const currentConversation = conversationList.find((item) => item.key === curConversation)
    const remainingConversations = conversationList.filter((item) => item.key !== curConversation)

    return currentConversation
      ? [currentConversation, ...remainingConversations]
      : remainingConversations
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
      let nextLabel = ""
      let didUpdateLabel = false
      const nextList = conversationList.map((item) => {
        const label = item.label ?? "New chat"

        if (item.key !== conversationKey) {
          return item
        }

        if (label !== "New chat" && label !== "New Conversation") {
          return item
        }

        nextLabel = createConversationLabel(prompt)
        didUpdateLabel = true
        return {
          ...item,
          label: nextLabel,
        }
      })

      if (didUpdateLabel && nextLabel) {
        setStoredSessionLabel(conversationKey, nextLabel)
      }

      setConversations(nextList)
    },
    [conversationList, setConversations]
  )

  const handleCreateConversation = useCallback(async () => {
    if (isSessionBusy) {
      toast.info("Wait for the current response to finish before changing sessions.")
      return
    }

    if (safeMessages.length === 0 && curConversation) {
      toast.info("The current session is already empty.")
      return
    }

    const session = await createEmptySession({ select: true })
    if (session) {
      setDraft("")
    }
  }, [createEmptySession, curConversation, isSessionBusy, safeMessages.length])

  const handleDeleteConversation = useCallback(
    async (conversationKey: string) => {
      if (isSessionBusy) {
        toast.info("Wait for the current response to finish before deleting sessions.")
        return
      }

      try {
        await deleteSessionMutation.mutateAsync({
          params: {
            path: { id: conversationKey },
          },
        })
      } catch (error) {
        toast.error("Failed to delete chat session.", {
          description: getErrorDescription(error),
        })
        return
      }

      removeStoredSessionLabel(conversationKey)
      clearConversationCache(conversationKey)

      const nextList = conversationList.filter((item) => item.key !== conversationKey)
      setConversations(nextList)

      if (nextList.length === 0) {
        setCurConversation("")
        await createEmptySession({ select: true })
        return
      }

      if (conversationKey === curConversation) {
        setCurConversation(nextList[0]?.key ?? "")
      }

      void refetchSessions()
    },
    [
      conversationList,
      createEmptySession,
      curConversation,
      deleteSessionMutation,
      isSessionBusy,
      refetchSessions,
      setConversations,
    ]
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
      if (!curConversation || isSessionBusy || isSessionBootstrapping) {
        toast.info("Chat session is still syncing. Please try again in a moment.")
        return
      }

      setConversationLabelIfNeeded(curConversation, value)
      setDraft("")
      await handleSubmit(value)
    },
    [
      curConversation,
      handleSubmit,
      isSessionBootstrapping,
      isSessionBusy,
      setConversationLabelIfNeeded,
    ]
  )

  return (
    <XProvider locale={locale}>
      <ChatContext.Provider value={{ onReload }}>
        <div className="relative flex min-h-0 flex-1 flex-col bg-[var(--shell-card)]">
          <div className="pointer-events-none absolute right-4 top-4 z-20 hidden xl:block">
            <div className="pointer-events-auto">
              <ChatSessionSummaryCard
                items={sessionSummaryItems}
                onManageSessions={() => setIsSessionSheetOpen(true)}
                onNewSession={handleCreateConversation}
                disableNewSession={isSessionBusy || isSessionBootstrapping}
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
              disableNewSession={isSessionBusy || isSessionBootstrapping}
            />
          </div>

          <ScrollArea className="min-h-0 flex-1">
            <div className="mx-auto flex w-full max-w-[682px] flex-col gap-8 px-6 pb-24 pt-2 md:px-8 md:pb-28 xl:px-0">
              {isSessionBootstrapping || (isHistoryLoading && safeMessages.length === 0) ? (
                <div className="flex min-h-[260px] items-center justify-center rounded-[32px] border border-dashed border-border/60 bg-[linear-gradient(180deg,color-mix(in_oklab,var(--app-canvas)_90%,transparent)_0%,color-mix(in_oklab,var(--app-canvas)_50%,transparent)_100%)] px-8 text-center">
                  <div className="max-w-md space-y-3">
                    <p className="text-base font-medium text-foreground">Loading this session...</p>
                    <p className="text-sm leading-6 text-muted-foreground">
                      Restoring the saved conversation history before you continue.
                    </p>
                  </div>
                </div>
              ) : safeMessages.length === 0 ? (
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
                    onContinue={item.id === latestContinuableMessageId ? onContinue : undefined}
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

          <div className="relative shrink-0 bg-[var(--shell-card)]">
            <div className="pointer-events-none absolute inset-x-0 top-0 h-20 -translate-y-full bg-gradient-to-b from-transparent via-[color-mix(in_oklab,var(--shell-card)_92%,transparent)] to-[var(--shell-card)]" />
            <div className="relative mx-auto w-full max-w-[768px] px-6 pb-6 pt-4 md:px-8 xl:px-0">
              <ChatComposer
                value={draft}
                onValueChange={setDraft}
                onSubmit={submitChatMessage}
                onCancel={abort}
                isRequesting={isRequesting || isPreparingModel}
                disabled={isSessionBootstrapping || isHistoryLoading || isSessionMutating || !curConversation}
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
            busy={isSessionBusy || isSessionBootstrapping}
            onSelect={(key) => {
              if (isSessionBusy || isSessionBootstrapping) {
                return
              }
              setCurConversation(key)
              setIsSessionSheetOpen(false)
            }}
            onDelete={handleDeleteConversation}
          />
        </div>
      </ChatContext.Provider>
    </XProvider>
  )
}

export default Chat

function formatProviderLabel(provider: string) {
  return provider
    .replace(/^cloud\./, "")
    .replace(/^local\./, "")
    .replace(/[._-]+/g, " ")
    .replace(/\b\w/g, (char) => char.toUpperCase())
}
