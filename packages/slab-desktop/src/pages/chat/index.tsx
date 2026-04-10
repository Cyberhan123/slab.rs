import { XProvider } from "@ant-design/x"
import { Loader2 } from "lucide-react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { useNavigate } from "react-router-dom"
import { toast } from "sonner"

import "@ant-design/x-markdown/themes/dark.css"
import "@ant-design/x-markdown/themes/light.css"

import { Button } from "@slab/components/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@slab/components/dialog"
import { ScrollArea } from "@slab/components/scroll-area"
import { ChatComposer } from "@/pages/chat/components/chat-composer"
import { ChatMessageBubble } from "@/pages/chat/components/chat-message-bubble"
import { ChatSessionSheet } from "@/pages/chat/components/chat-session-sheet"
import {
  ChatSessionSummaryCard,
  type ChatSessionSummaryItem,
} from "@/pages/chat/components/chat-session-summary-card"
import api from "@/lib/api"
import { toCatalogModelList, type CatalogModel } from "@/lib/api/models"
import { PAGE_HEADER_META } from "@/layouts/header-meta"
import { usePageHeader, usePageHeaderControl } from "@/hooks/use-global-header-meta"
import { usePersistedHeaderSelect } from "@/hooks/use-persisted-header-select"
import { HEADER_SELECT_KEYS } from "@/layouts/header-controls"
import { useChatUiStore } from "@/store/useChatUiStore"

import {
  ChatContext,
  getChatMessageTextContent,
  type ChatMessageRecord,
} from "./chat-context"
import { useChat } from "./hooks/use-chat"
import { useChatSessions } from "./hooks/use-chat-sessions"
import { useMarkdownTheme } from "./hooks/use-markdowm-theme"
import locale from "./local"

const MODEL_DOWNLOAD_POLL_INTERVAL_MS = 2_000
const MODEL_DOWNLOAD_TIMEOUT_MS = 30 * 60 * 1_000

type ModelOptionSource = "local" | "cloud"

type ChatModelCapabilities = {
  raw_grammar: boolean
  structured_output: boolean
  reasoning_controls: boolean
}

type ModelOption = {
  id: string
  label: string
  downloaded: boolean
  pending: boolean
  source: ModelOptionSource
  capabilities: ChatModelCapabilities
  contextWindow?: number | null
}

function createConversationLabel(value: string) {
  const trimmed = value.trim()

  if (!trimmed) {
    return "New chat"
  }

  return trimmed.length > 42 ? `${trimmed.slice(0, 42)}...` : trimmed
}

function defaultCapabilitiesForSource(source: ModelOptionSource): ChatModelCapabilities {
  return source === "cloud"
    ? {
        raw_grammar: false,
        structured_output: true,
        reasoning_controls: true,
      }
    : {
        raw_grammar: true,
        structured_output: true,
        reasoning_controls: false,
      }
}

function resolveChatModelCapabilities(
  model: Pick<CatalogModel, "chat_capabilities" | "kind">
): ChatModelCapabilities {
  return model.chat_capabilities ?? defaultCapabilitiesForSource(model.kind)
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
  const [draft, setDraft] = useState("")
  const [isSessionSheetOpen, setIsSessionSheetOpen] = useState(false)
  const [pendingModelSwitchId, setPendingModelSwitchId] = useState<string | null>(null)
  const [loadedModelId, setLoadedModelId] = useState<string | null>(null)
  const bottomRef = useRef<HTMLDivElement | null>(null)
  const deepThink = useChatUiStore((state) => state.deepThink)
  const setDeepThink = useChatUiStore((state) => state.setDeepThink)
  const {
    conversationList,
    createSession: createEmptySession,
    currentSessionId: curConversation,
    deleteSession: deleteConversationSession,
    isCreatingSession,
    isDeletingSession,
    isSessionMutating,
    isSessionsLoading: sessionsLoading,
    setCurrentSessionId: setCurConversation,
    setSessionLabel,
  } = useChatSessions()

  const {
    data: catalogModels,
    isLoading: catalogModelsLoading,
    refetch: refetchCatalogModels,
  } = api.useQuery("get", "/v1/models", {
    params: {
      query: {
        capability: "chat_generation",
      },
    },
  })

  const downloadModelMutation = api.useMutation("post", "/v1/models/download")
  const loadModelMutation = api.useMutation("post", "/v1/models/load")
  const switchModelMutation = api.useMutation("post", "/v1/models/switch")
  const getTaskMutation = api.useMutation("get", "/v1/tasks/{id}")

  const parsedCatalogModels = useMemo(
    () => toCatalogModelList(catalogModels),
    [catalogModels]
  )

  const localChatModels = useMemo(
    () => parsedCatalogModels.filter((model) => model.kind === "local"),
    [parsedCatalogModels]
  )

  const modelOptions = useMemo<ModelOption[]>(
    () =>
      parsedCatalogModels.map((model) => {
        const downloaded =
          model.kind === "cloud" ||
          (model.status === "ready" && typeof model.local_path === "string" && model.local_path.length > 0)

        return {
          id: model.id,
          label: model.display_name,
          downloaded,
          pending: model.pending,
          source: model.kind,
          capabilities: resolveChatModelCapabilities(model),
          contextWindow: model.spec.context_window ?? null,
        }
      }),
    [parsedCatalogModels]
  )
  const { value: selectedModelId, setValue: setSelectedModelId } = usePersistedHeaderSelect({
    key: HEADER_SELECT_KEYS.chatModel,
    options: modelOptions,
    isLoading: catalogModelsLoading,
  })
  const selectedModel = useMemo(
    () => modelOptions.find((item) => item.id === selectedModelId),
    [modelOptions, selectedModelId]
  )
  const pendingModelSwitch = useMemo(
    () => modelOptions.find((item) => item.id === pendingModelSwitchId) ?? null,
    [modelOptions, pendingModelSwitchId]
  )

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
    let model = localChatModels.find((item) => item.id === modelId)
    if (!model) {
      model = await refreshCatalogAndFindModel(modelId)
    }

    if (!model) {
      throw new Error("Selected model does not exist in catalog")
    }

    if (model.kind !== "local") {
      throw new Error("Selected model is not a local chat model")
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

    const selectedLocal = localChatModels.find((item) => item.id === selectedModelId)
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
  } = useChat(
    curConversation,
    selectedModelId || "slab-llama",
    deepThink,
    selectedModel?.capabilities.reasoning_controls ?? false,
    ensureChatModelReady
  )

  const modelLoading = catalogModelsLoading
  const isPreparingModel =
    loadModelMutation.isPending ||
    switchModelMutation.isPending ||
    downloadModelMutation.isPending
  const isSessionBusy = isRequesting || isPreparingModel || isHistoryLoading || isSessionMutating
  const isSessionBootstrapping = (sessionsLoading || isCreatingSession) && conversationList.length === 0

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
  useEffect(() => {
    if (selectedModel && !selectedModel.capabilities.reasoning_controls && deepThink) {
      setDeepThink(false)
    }
  }, [deepThink, selectedModel])

  const latestUserMessage = safeMessages
    .slice()
    .reverse()
    .find((item) => item.message.role === "user")
  const currentConversationLabel =
    conversationList.find((item) => item.key === curConversation)?.label?.trim() || "Current session"
  const selectedModelStatusLabel = useMemo(() => {
    if (isSessionBootstrapping || !curConversation) {
      return "Preparing session"
    }

    if (isHistoryLoading) {
      return "Loading session history"
    }

    if (isCreatingSession) {
      return "Creating session"
    }

    if (isDeletingSession) {
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
    curConversation,
    isCreatingSession,
    isDeletingSession,
    isHistoryLoading,
    isPreparingModel,
    isSessionBootstrapping,
    modelLoading,
    selectedModel,
  ])

  const closePendingModelSwitch = useCallback(() => {
    if (isCreatingSession) {
      return
    }

    setPendingModelSwitchId(null)
  }, [isCreatingSession])

  const handleModelPickerChange = useCallback(
    (nextModelId: string) => {
      if (!nextModelId || nextModelId === selectedModelId) {
        return
      }

      if (isSessionBusy || isSessionBootstrapping) {
        toast.info("Wait for the current response or session sync to finish before switching models.")
        return
      }

      if (!curConversation || safeMessages.length === 0) {
        setSelectedModelId(nextModelId)
        return
      }

      setPendingModelSwitchId(nextModelId)
    },
    [curConversation, isSessionBootstrapping, isSessionBusy, safeMessages.length, selectedModelId]
  )

  const handleKeepSessionOnModelSwitch = useCallback(() => {
    if (!pendingModelSwitchId) {
      return
    }

    setSelectedModelId(pendingModelSwitchId)
    setPendingModelSwitchId(null)
  }, [pendingModelSwitchId])

  const handleCreateSessionOnModelSwitch = useCallback(async () => {
    if (!pendingModelSwitchId) {
      return
    }

    const nextModelId = pendingModelSwitchId
    const session = await createEmptySession({ select: true })

    if (!session) {
      return
    }

    setSelectedModelId(nextModelId)
    setPendingModelSwitchId(null)
  }, [createEmptySession, pendingModelSwitchId])

  const headerModelPicker = useMemo(
    () => ({
      type: "select" as const,
      value: selectedModelId,
      options: modelOptions.map((model) => ({
        id: model.id,
        label: model.label,
      })),
      onValueChange: handleModelPickerChange,
      groupLabel: "Chat Models",
      placeholder: "Select model",
      loading: modelLoading,
      disabled:
        modelLoading ||
        isSessionBusy ||
        isSessionBootstrapping ||
        Boolean(pendingModelSwitchId) ||
        modelOptions.length === 0,
      emptyLabel: "No chat models",
    }),
    [
      handleModelPickerChange,
      isSessionBootstrapping,
      isSessionBusy,
      modelLoading,
      modelOptions,
      pendingModelSwitchId,
      selectedModelId,
    ]
  )
  const latestUserPrompt = getChatMessageTextContent(latestUserMessage?.message).trim()

  usePageHeader(PAGE_HEADER_META.chat)
  usePageHeaderControl(headerModelPicker)

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
      const conversation = conversationList.find((item) => item.key === conversationKey)
      const label = conversation?.label ?? "New chat"

      if (label !== "New chat" && label !== "New Conversation") {
        return
      }

      const nextLabel = createConversationLabel(prompt)
      if (nextLabel) {
        setSessionLabel(conversationKey, nextLabel)
      }
    },
    [conversationList, setSessionLabel]
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

      await deleteConversationSession(conversationKey)
    },
    [deleteConversationSession, isSessionBusy]
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
                reasoningSupported={selectedModel?.capabilities.reasoning_controls ?? false}
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

          <Dialog
            open={Boolean(pendingModelSwitchId)}
            onOpenChange={(open) => {
              if (!open) {
                closePendingModelSwitch()
              }
            }}
          >
            <DialogContent className="max-w-xl" showCloseButton={!isCreatingSession}>
              <DialogHeader className="space-y-3 text-left">
                <DialogTitle>Switch model for this conversation?</DialogTitle>
                <DialogDescription>
                  Choose whether the new model should keep using this session history or start from
                  a clean session.
                </DialogDescription>
              </DialogHeader>

              <div className="space-y-2 text-sm leading-6 text-muted-foreground">
                <p>
                  You are switching from <strong>{selectedModel?.label ?? "the current model"}</strong> to{" "}
                  <strong>{pendingModelSwitch?.label ?? pendingModelSwitchId ?? "the selected model"}</strong>.
                </p>
                <p>
                  <strong>{currentConversationLabel}</strong> already has {safeMessages.length}{" "}
                  {safeMessages.length === 1 ? "message" : "messages"}.
                </p>
              </div>

              <div className="grid gap-3 sm:grid-cols-2">
                <div className="rounded-2xl border border-border/70 bg-[var(--surface-1)] px-4 py-3">
                  <p className="text-sm font-medium text-foreground">Keep current session</p>
                  <p className="mt-1 text-sm leading-6 text-muted-foreground">
                    The new model will continue from this conversation and see the existing message
                    history.
                  </p>
                </div>
                <div className="rounded-2xl border border-border/70 bg-[var(--surface-1)] px-4 py-3">
                  <p className="text-sm font-medium text-foreground">Create new session</p>
                  <p className="mt-1 text-sm leading-6 text-muted-foreground">
                    Start with a clean session and keep the previous conversation attached to the old
                    model.
                  </p>
                </div>
              </div>

              <DialogFooter className="gap-2">
                <Button
                  variant="outline"
                  onClick={closePendingModelSwitch}
                  disabled={isCreatingSession}
                >
                  Cancel
                </Button>
                <Button
                  variant="secondary"
                  onClick={handleKeepSessionOnModelSwitch}
                  disabled={isCreatingSession}
                >
                  Keep current session
                </Button>
                <Button
                  onClick={() => void handleCreateSessionOnModelSwitch()}
                  disabled={isCreatingSession}
                >
                  {isCreatingSession ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : null}
                  Create new session
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </div>
      </ChatContext.Provider>
    </XProvider>
  )
}

export default Chat
