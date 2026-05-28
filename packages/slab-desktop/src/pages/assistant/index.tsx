import { Bubble, XProvider, type BubbleListProps } from "@ant-design/x"
import { Loader2 } from "lucide-react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { useNavigate } from "react-router-dom"
import { toast } from "sonner"

import "@ant-design/x-markdown/themes/dark.css"
import "@ant-design/x-markdown/themes/light.css"

import { Button } from "@slab/components/button"
import {
  DEFAULT_ASSISTANT_LABELS,
  LEGACY_DEFAULT_CHAT_LABELS,
  Trans,
  getResolvedAppLanguage,
  useTranslation,
} from "@slab/i18n"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@slab/components/dialog"
import { ScrollArea } from "@slab/components/scroll-area"
import api from "@slab/api"
import { toCatalogModelList, type CatalogModel } from "@slab/api/models"
import { usePageHeader, usePageHeaderControl } from "@/hooks/use-global-header-meta"
import { usePersistedHeaderSelect } from "@/hooks/use-persisted-header-select"
import { PAGE_HEADER_META } from "@/layouts/header-meta"
import { HEADER_SELECT_KEYS } from "@/layouts/header-controls"
import { useAssistantUiStore } from "@/store/useAssistantUiStore"
import {
  extractTaskId,
  isFailedTaskStatus,
  MODEL_DOWNLOAD_POLL_INTERVAL_MS,
  MODEL_DOWNLOAD_TIMEOUT_MS,
  sleep,
} from "@/pages/task/utils"

import {
  getAssistantErrorDescription,
  getAssistantMessageTextContent,
  type AssistantMessageRecord,
} from "./assistant-context"
import { AssistantComposer } from "./components/assistant-composer"
import { AssistantMessageBubble } from "./components/assistant-message-bubble"
import { AssistantSessionSheet } from "./components/assistant-session-sheet"
import {
  AssistantSessionSummaryCard,
  type AssistantSessionSummaryItem,
} from "./components/assistant-session-summary-card"
import { useAssistantLocale } from "./assistant-locale"
import { useAssistantAgent } from "./hooks/use-assistant-agent"
import { useAssistantSessions } from "./hooks/use-assistant-sessions"
import { useMarkdownTheme } from "./hooks/use-markdown-theme"

type ModelOptionSource = "local" | "cloud"

type AssistantModelCapabilities = {
  raw_gbnf: boolean
  structured_output: boolean
  reasoning_controls: boolean
}

type ModelOption = {
  id: string
  label: string
  downloaded: boolean
  pending: boolean
  source: ModelOptionSource
  capabilities: AssistantModelCapabilities
  contextWindow?: number | null
  runtimePresets?: CatalogModel["runtime_presets"]
}

type AssistantBubbleContent = {
  approving: boolean
  item: AssistantMessageRecord
  markdownClassName?: string
  onApprove?: (approved: boolean) => void
}

function renderAssistantBubbleContent(content: AssistantBubbleContent) {
  return (
    <AssistantMessageBubble
      approving={content.approving}
      item={content.item}
      markdownClassName={content.markdownClassName}
      onApprove={content.onApprove}
    />
  )
}

const ASSISTANT_BUBBLE_ROLES = {
  assistant: {
    contentRender: renderAssistantBubbleContent,
    variant: "borderless",
  },
  user: {
    contentRender: renderAssistantBubbleContent,
    variant: "borderless",
  },
} satisfies BubbleListProps["role"]

function createConversationLabel(value: string, fallback: string) {
  const trimmed = value.trim()

  if (!trimmed) {
    return fallback
  }

  return trimmed.length > 42 ? `${trimmed.slice(0, 42)}...` : trimmed
}

function defaultCapabilitiesForSource(source: ModelOptionSource): AssistantModelCapabilities {
  return source === "cloud"
    ? {
      raw_gbnf: false,
      reasoning_controls: true,
      structured_output: true,
    }
    : {
      raw_gbnf: true,
      reasoning_controls: false,
      structured_output: true,
    }
}

function resolveAssistantModelCapabilities(
  model: Pick<CatalogModel, "chat_capabilities" | "kind">
): AssistantModelCapabilities {
  return model.chat_capabilities ?? defaultCapabilitiesForSource(model.kind)
}

function getGreeting(date: Date, t: (key: string) => string) {
  const hour = date.getHours()

  if (hour < 12) {
    return t("pages.assistant.greeting.morning")
  }

  if (hour < 18) {
    return t("pages.assistant.greeting.afternoon")
  }

  return t("pages.assistant.greeting.evening")
}

function Assistant() {
  const navigate = useNavigate()
  const [markdownThemeClassName] = useMarkdownTheme()
  const [draft, setDraft] = useState("")
  const [isSessionSheetOpen, setIsSessionSheetOpen] = useState(false)
  const [pendingModelSwitchId, setPendingModelSwitchId] = useState<string | null>(null)
  const [loadedModelId, setLoadedModelId] = useState<string | null>(null)
  const bottomRef = useRef<HTMLDivElement | null>(null)
  const deepThink = useAssistantUiStore((state) => state.deepThink)
  const setDeepThink = useAssistantUiStore((state) => state.setDeepThink)
  const { t } = useTranslation()
  const locale = useAssistantLocale()
  const resolvedLanguage = getResolvedAppLanguage()
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
  } = useAssistantSessions()

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

  const localAssistantModels = useMemo(
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
          capabilities: resolveAssistantModelCapabilities(model),
          contextWindow: model.spec.context_window ?? null,
          downloaded,
          id: model.id,
          label: model.display_name,
          pending: model.pending,
          runtimePresets: model.runtime_presets ?? null,
          source: model.kind,
        }
      }),
    [parsedCatalogModels]
  )
  const { value: selectedModelId, setValue: setSelectedModelId } = usePersistedHeaderSelect({
    key: HEADER_SELECT_KEYS.assistantModel,
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

  const waitForTaskToFinish = async (taskId: string) => {
    const deadline = Date.now() + MODEL_DOWNLOAD_TIMEOUT_MS

    while (Date.now() < deadline) {
      // eslint-disable-next-line no-await-in-loop
      const task = (await getTaskMutation.mutateAsync({
        params: { path: { id: taskId } },
      })) as { status: string; error_msg?: string | null }

      if (task.status === "succeeded") {
        return
      }

      if (isFailedTaskStatus(task.status)) {
        throw new Error(task.error_msg ?? `Task ${taskId} ended with status: ${task.status}`)
      }

      // eslint-disable-next-line no-await-in-loop
      await sleep(MODEL_DOWNLOAD_POLL_INTERVAL_MS)
    }

    throw new Error(t("pages.assistant.error.downloadTimedOut"))
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
    let model = localAssistantModels.find((item) => item.id === modelId)
    if (!model) {
      model = await refreshCatalogAndFindModel(modelId)
    }

    if (!model) {
      throw new Error(t("pages.assistant.error.selectedModelMissing"))
    }

    if (model.kind !== "local") {
      throw new Error(t("pages.assistant.error.selectedModelNotLocal"))
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
      throw new Error(t("pages.assistant.error.selectModelFirst"))
    }

    if (loadedModelId === selectedModelId) {
      return
    }

    const selectedOption = modelOptions.find((item) => item.id === selectedModelId)
    if (!selectedOption) {
      throw new Error(t("pages.assistant.error.selectedModelUnavailable"))
    }

    if (selectedOption.source === "cloud") {
      setLoadedModelId(selectedModelId)
      return
    }

    const selectedLocal = localAssistantModels.find((item) => item.id === selectedModelId)
    const { downloadedNow } = await ensureDownloadedModelPath(selectedModelId)

    if (downloadedNow) {
      toast.success(
        t("pages.assistant.toast.downloaded", {
          model: selectedLocal?.display_name ?? selectedModelId,
        })
      )
    }

    try {
      await loadOrSwitchSelectedModel(selectedModelId)
    } catch (firstLoadError) {
      if (downloadedNow) {
        throw firstLoadError
      }

      toast.message(t("pages.assistant.toast.modelLoadRetry"))

      const retry = await ensureDownloadedModelPath(selectedModelId, true)
      if (retry.downloadedNow) {
        toast.success(
          t("pages.assistant.toast.downloaded", {
            model: selectedLocal?.display_name ?? selectedModelId,
          })
        )
      }

      await loadOrSwitchSelectedModel(selectedModelId)
    }

    setLoadedModelId(selectedModelId)
  }

  const ensureAssistantModelReady = async () => {
    try {
      await prepareSelectedModel()
    } catch (error) {
      toast.error(t("pages.assistant.toast.failedToPrepareModel"), {
        description: getAssistantErrorDescription(error, t("pages.assistant.toast.unknownError")),
      })
      throw error
    }
  }

  const {
    activeConversation,
    abort,
    eventsConnected,
    handleSubmit,
    isHistoryLoading,
    isRequesting,
    messages,
    submitApproval,
  } = useAssistantAgent({
    beforeRequest: ensureAssistantModelReady,
    deepThink,
    model: selectedModelId || "slab-llama",
    runtimePresets: selectedModel?.runtimePresets ?? null,
    sessionId: curConversation,
  })

  const modelLoading = catalogModelsLoading
  const isPreparingModel =
    loadModelMutation.isPending ||
    switchModelMutation.isPending ||
    downloadModelMutation.isPending
  const isSessionBusy = isRequesting || isPreparingModel || isHistoryLoading || isSessionMutating
  const isSessionBootstrapping = (sessionsLoading || isCreatingSession) && conversationList.length === 0
  const safeMessages = useMemo<AssistantMessageRecord[]>(() => messages ?? [], [messages])

  useEffect(() => {
    if (selectedModel && !selectedModel.capabilities.reasoning_controls && deepThink) {
      setDeepThink(false)
    }
  }, [deepThink, selectedModel, setDeepThink])

  const latestUserMessage = safeMessages
    .slice()
    .toReversed()
    .find((item) => item.message.role === "user")
  const currentConversationLabel =
    conversationList.find((item) => item.key === curConversation)?.label?.trim() ||
    t("pages.assistant.sessionSummary.currentSession")
  const selectedModelStatusLabel = useMemo(() => {
    if (isSessionBootstrapping || !curConversation) {
      return t("pages.assistant.status.preparingSession")
    }

    if (isHistoryLoading) {
      return t("pages.assistant.status.loadingSessionHistory")
    }

    if (isCreatingSession) {
      return t("pages.assistant.status.creatingSession")
    }

    if (isDeletingSession) {
      return t("pages.assistant.status.deletingSession")
    }

    if (modelLoading) {
      return t("pages.assistant.status.loadingModels")
    }

    if (!selectedModel) {
      return t("pages.assistant.status.selectModel")
    }

    const parts = [selectedModel.label]

    if (selectedModel.contextWindow && selectedModel.contextWindow > 0) {
      parts.push(
        t("pages.assistant.status.contextWindow", {
          formatted: new Intl.NumberFormat(resolvedLanguage).format(selectedModel.contextWindow),
        })
      )
    } else if (selectedModel.pending) {
      parts.push(t("pages.assistant.status.downloading"))
    } else if (selectedModel.source === "local" && !selectedModel.downloaded) {
      parts.push(t("pages.assistant.status.needsDownload"))
    } else if (isPreparingModel) {
      parts.push(t("pages.assistant.status.preparing"))
    } else if (selectedModel.source === "cloud") {
      parts.push(t("pages.assistant.status.cloudModel"))
    }

    if (eventsConnected) {
      parts.push(t("pages.assistant.connection.connected"))
    }

    return parts.join(" / ")
  }, [
    curConversation,
    eventsConnected,
    isCreatingSession,
    isDeletingSession,
    isHistoryLoading,
    isPreparingModel,
    isSessionBootstrapping,
    modelLoading,
    resolvedLanguage,
    selectedModel,
    t,
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
        toast.info(t("pages.assistant.toast.waitBeforeSwitchingModels"))
        return
      }

      if (!curConversation || safeMessages.length === 0) {
        setSelectedModelId(nextModelId)
        return
      }

      setPendingModelSwitchId(nextModelId)
    },
    [
      curConversation,
      isSessionBootstrapping,
      isSessionBusy,
      safeMessages.length,
      selectedModelId,
      setSelectedModelId,
      t,
    ]
  )

  const handleKeepSessionOnModelSwitch = useCallback(() => {
    if (!pendingModelSwitchId) {
      return
    }

    setSelectedModelId(pendingModelSwitchId)
    setPendingModelSwitchId(null)
  }, [pendingModelSwitchId, setSelectedModelId])

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
  }, [createEmptySession, pendingModelSwitchId, setSelectedModelId])

  const headerModelPicker = useMemo(
    () => ({
      disabled:
        modelLoading ||
        isSessionBusy ||
        isSessionBootstrapping ||
        Boolean(pendingModelSwitchId) ||
        modelOptions.length === 0,
      emptyLabel: t("pages.assistant.modelPicker.emptyLabel"),
      groupLabel: t("pages.assistant.modelPicker.groupLabel"),
      loading: modelLoading,
      onValueChange: handleModelPickerChange,
      options: modelOptions.map((model) => ({
        id: model.id,
        label: model.label,
      })),
      placeholder: t("pages.assistant.modelPicker.placeholder"),
      type: "select" as const,
      value: selectedModelId,
    }),
    [
      handleModelPickerChange,
      isSessionBootstrapping,
      isSessionBusy,
      modelLoading,
      modelOptions,
      pendingModelSwitchId,
      selectedModelId,
      t,
    ]
  )
  const latestUserPrompt = getAssistantMessageTextContent(latestUserMessage?.message).trim()

  usePageHeader({
    ...PAGE_HEADER_META.assistant,
    title: t("pages.assistant.header.title"),
    subtitle: t("pages.assistant.header.subtitle"),
  })
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
  const greeting = useMemo(() => getGreeting(new Date(), t), [t])
  const sessionSummaryItems = useMemo<AssistantSessionSummaryItem[]>(() => {
    return sortedConversations.slice(0, 2).map<AssistantSessionSummaryItem>((conversation, index) => ({
      hint:
        conversation.key === curConversation
          ? t("pages.assistant.sessionSummary.messageCount", { count: safeMessages.length })
          : conversation.group ?? t("pages.assistant.runtime.workspace"),
      key: conversation.key,
      label:
        conversation.label ??
        t(
          index === 0
            ? "pages.assistant.sessionSummary.currentSession"
            : "pages.assistant.sessionSummary.nextSession"
        ),
      tone: index === 0 ? "warm" : "mint",
    }))
  }, [curConversation, safeMessages.length, sortedConversations, t])

  const setConversationLabelIfNeeded = useCallback(
    (conversationKey: string, prompt: string) => {
      const conversation = conversationList.find((item) => item.key === conversationKey)
      const label = conversation?.label ?? t("pages.assistant.runtime.newChat")
      const defaultLabels = new Set([
        t("pages.assistant.runtime.newChat"),
        t("pages.assistant.runtime.newConversation"),
        ...DEFAULT_ASSISTANT_LABELS,
        ...LEGACY_DEFAULT_CHAT_LABELS,
      ])

      if (!defaultLabels.has(label)) {
        return
      }

      const nextLabel = createConversationLabel(prompt, t("pages.assistant.runtime.newChat"))
      if (nextLabel) {
        setSessionLabel(conversationKey, nextLabel)
      }
    },
    [conversationList, setSessionLabel, t]
  )

  const handleCreateConversation = useCallback(async () => {
    if (isSessionBusy) {
      toast.info(t("pages.assistant.toast.waitForCurrentResponse"))
      return
    }

    if (safeMessages.length === 0 && curConversation) {
      toast.info(t("pages.assistant.toast.currentSessionAlreadyEmpty"))
      return
    }

    const session = await createEmptySession({ select: true })
    if (session) {
      setDraft("")
    }
  }, [createEmptySession, curConversation, isSessionBusy, safeMessages.length, t])

  const handleDeleteConversation = useCallback(
    async (conversationKey: string) => {
      if (isSessionBusy) {
        toast.info(t("pages.assistant.toast.waitBeforeDeletingSessions"))
        return
      }

      await deleteConversationSession(conversationKey)
    },
    [deleteConversationSession, isSessionBusy, t]
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

  const submitAssistantMessage = useCallback(
    async (value: string) => {
      if (!curConversation || isSessionBusy || isSessionBootstrapping) {
        toast.info(t("pages.assistant.toast.sessionSyncing"))
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
      t,
    ]
  )

  const bubbleItems = useMemo(
    () =>
      safeMessages.map((item) => ({
        content: {
          approving: isRequesting,
          item,
          markdownClassName: markdownThemeClassName,
          onApprove: submitApproval,
        },
        key: item.id,
        role: item.message.role === "assistant" ? "assistant" : "user",
        status: item.status,
      })),
    [isRequesting, markdownThemeClassName, safeMessages, submitApproval]
  )

  return (
    <XProvider locale={locale}>
      <div className="relative flex min-h-0 flex-1 flex-col bg-[var(--shell-card)]">
        <div className="pointer-events-none absolute right-4 top-4 z-20 hidden lg:block">
          <div className="pointer-events-auto">
            <AssistantSessionSummaryCard
              items={sessionSummaryItems}
              onManageSessions={() => setIsSessionSheetOpen(true)}
              onNewSession={handleCreateConversation}
              disableNewSession={isSessionBusy || isSessionBootstrapping}
            />
          </div>
        </div>

        <div className="mx-auto w-full max-w-[768px] px-6 pb-6 pt-12 md:px-8 lg:px-0">
          <div className="space-y-2">
            <h1 className="text-[clamp(2.75rem,6vw,4rem)] font-semibold tracking-[-0.055em] text-foreground">
              {greeting}
            </h1>
            <p className="text-lg leading-7 text-muted-foreground/80">
              {t("pages.assistant.hero.description")}
            </p>
          </div>
        </div>

        <div className="mx-auto block w-full max-w-[768px] px-6 pb-6 md:px-8 lg:hidden lg:px-0">
          <AssistantSessionSummaryCard
            items={sessionSummaryItems}
            onManageSessions={() => setIsSessionSheetOpen(true)}
            onNewSession={handleCreateConversation}
            disableNewSession={isSessionBusy || isSessionBootstrapping}
          />
        </div>

        <ScrollArea className="min-h-0 flex-1">
          <div className="mx-auto flex w-full max-w-[682px] flex-col gap-8 px-6 pb-24 pt-2 md:px-8 md:pb-28 lg:px-0">
            {isSessionBootstrapping || (isHistoryLoading && safeMessages.length === 0) ? (
              <div className="flex min-h-[260px] items-center justify-center rounded-[32px] border border-dashed border-border/60 bg-[linear-gradient(180deg,color-mix(in_oklab,var(--app-canvas)_90%,transparent)_0%,color-mix(in_oklab,var(--app-canvas)_50%,transparent)_100%)] px-8 text-center">
                <div className="max-w-md space-y-3">
                  <p className="text-base font-medium text-foreground">
                    {t("pages.assistant.loading.title")}
                  </p>
                  <p className="text-sm leading-6 text-muted-foreground">
                    {t("pages.assistant.loading.description")}
                  </p>
                </div>
              </div>
            ) : safeMessages.length === 0 ? (
              <div className="flex min-h-[260px] items-center justify-center rounded-[32px] border border-dashed border-border/60 bg-[linear-gradient(180deg,color-mix(in_oklab,var(--app-canvas)_90%,transparent)_0%,color-mix(in_oklab,var(--app-canvas)_50%,transparent)_100%)] px-8 text-center">
                <div className="max-w-md space-y-3">
                  <p className="text-base font-medium text-foreground">
                    {t("pages.assistant.emptyState.title")}
                  </p>
                  <p className="text-sm leading-6 text-muted-foreground">
                    {t("pages.assistant.emptyState.description")}
                  </p>
                </div>
              </div>
            ) : (
              <Bubble.List
                items={bubbleItems}
                role={ASSISTANT_BUBBLE_ROLES}
                autoScroll
                className="flex flex-col gap-8"
              />
            )}
            <div ref={bottomRef} />
          </div>
        </ScrollArea>

        <div className="relative shrink-0 bg-[var(--shell-card)]">
          <div className="relative mx-auto w-full max-w-[768px] px-6 pb-6 pt-4 md:px-8 lg:px-0">
            <AssistantComposer
              value={draft}
              onValueChange={setDraft}
              onSubmit={submitAssistantMessage}
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

        <AssistantSessionSheet
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
              <DialogTitle>{t("pages.assistant.dialog.title")}</DialogTitle>
              <DialogDescription>
                {t("pages.assistant.dialog.description")}
              </DialogDescription>
            </DialogHeader>

            <div className="space-y-2 text-sm leading-6 text-muted-foreground">
              <p>
                <Trans
                  i18nKey="pages.assistant.dialog.switchingSummary"
                  values={{
                    from: selectedModel?.label ?? t("pages.assistant.modelPicker.placeholder"),
                    to:
                      pendingModelSwitch?.label ??
                      pendingModelSwitchId ??
                      t("pages.assistant.modelPicker.placeholder"),
                  }}
                  components={{ strong: <strong /> }}
                />
              </p>
              <p>
                <Trans
                  i18nKey="pages.assistant.dialog.sessionSummary"
                  count={safeMessages.length}
                  values={{
                    count: safeMessages.length,
                    label: currentConversationLabel,
                  }}
                  components={{ strong: <strong /> }}
                />
              </p>
            </div>

            <div className="grid gap-3 sm:grid-cols-2">
              <div className="rounded-2xl border border-border/70 bg-[var(--surface-1)] px-4 py-3">
                <p className="text-sm font-medium text-foreground">
                  {t("pages.assistant.dialog.keepTitle")}
                </p>
                <p className="mt-1 text-sm leading-6 text-muted-foreground">
                  {t("pages.assistant.dialog.keepDescription")}
                </p>
              </div>
              <div className="rounded-2xl border border-border/70 bg-[var(--surface-1)] px-4 py-3">
                <p className="text-sm font-medium text-foreground">
                  {t("pages.assistant.dialog.createTitle")}
                </p>
                <p className="mt-1 text-sm leading-6 text-muted-foreground">
                  {t("pages.assistant.dialog.createDescription")}
                </p>
              </div>
            </div>

            <DialogFooter className="gap-2">
              <Button
                variant="outline"
                onClick={closePendingModelSwitch}
                disabled={isCreatingSession}
              >
                {t("pages.assistant.dialog.cancel")}
              </Button>
              <Button
                variant="secondary"
                onClick={handleKeepSessionOnModelSwitch}
                disabled={isCreatingSession}
              >
                {t("pages.assistant.dialog.keepTitle")}
              </Button>
              <Button
                onClick={() => void handleCreateSessionOnModelSwitch()}
                disabled={isCreatingSession}
              >
                {isCreatingSession ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : null}
                {t("pages.assistant.dialog.createTitle")}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </div>
    </XProvider>
  )
}

export default Assistant
