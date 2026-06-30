import {
  Bubble,
  XProvider,
  type BubbleListProps,
} from "@ant-design/x"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { useNavigate } from "react-router-dom"
import { toast } from "sonner"

import "@ant-design/x-markdown/themes/dark.css"
import "@ant-design/x-markdown/themes/light.css"

import {
  DEFAULT_ASSISTANT_LABELS,
  LEGACY_DEFAULT_CHAT_LABELS,
  getResolvedAppLanguage,
  useTranslation,
} from "@slab/i18n"
import { ScrollArea } from "@slab/components/scroll-area"
import api from "@slab/api"
import { toCatalogModelList } from "@slab/api/models"
import { usePageHeader, usePageHeaderControl } from "@/hooks/use-global-header-meta"
import { usePersistedHeaderSelect } from "@/hooks/use-persisted-header-select"
import { PAGE_HEADER_META } from "@/layouts/header-meta"
import { HEADER_SELECT_KEYS } from "@/layouts/header-controls"
import { useAssistantUiStore } from "@/store/useAssistantUiStore"
import { useAgentSurfaceStore } from "@/store/useAgentSurfaceStore"
import { GUARDRAIL_PMIDS, useGuardrailFlag } from "@/lib/guardrail-flags"
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
import { ASSISTANT_BUBBLE_ROLES } from "./components/assistant-bubble-content"
import { AssistantComposer } from "./components/assistant-composer"
import { AssistantModelSwitchDialog } from "./components/assistant-model-switch-dialog"
import { AssistantSessionSheet } from "./components/assistant-session-sheet"
import { AgentSurfaceLayer } from "./components/agent-surface-layer"
import {
  AssistantSessionSummaryCard,
  type AssistantSessionSummaryItem,
} from "./components/assistant-session-summary-card"
import { useAssistantLocale } from "./assistant-locale"
import { useAssistantAgent } from "./hooks/use-assistant-agent"
import { useAssistantSessions } from "./hooks/use-assistant-sessions"
import { useMarkdownTheme } from "./hooks/use-markdown-theme"
import {
  createConversationLabel,
  getGreeting,
  getSelectedModelStatusLabel,
  resolveAssistantModelCapabilities,
  type ModelOption,
  type ModelRuntimeStatus,
} from "./lib/assistant-page-state"

function Assistant() {
  const navigate = useNavigate()
  const [markdownThemeClassName] = useMarkdownTheme()
  const [draft, setDraft] = useState("")
  const [isSessionSheetOpen, setIsSessionSheetOpen] = useState(false)
  const [composerFocusSignal, setComposerFocusSignal] = useState(0)
  const [pendingModelSwitchId, setPendingModelSwitchId] = useState<string | null>(null)
  const [loadedModelId, setLoadedModelId] = useState<string | null>(null)
  const [loadedModelStatus, setLoadedModelStatus] = useState<ModelRuntimeStatus | null>(null)
  const bottomRef = useRef<HTMLDivElement | null>(null)
  const reasoningEffort = useAssistantUiStore((state) => state.reasoningEffort)
  const setReasoningEffort = useAssistantUiStore((state) => state.setReasoningEffort)
  const systemPrompt = useAssistantUiStore((state) => state.systemPrompt)
  const setSystemPrompt = useAssistantUiStore((state) => state.setSystemPrompt)
  const toolConcurrency = useAssistantUiStore((state) => state.toolConcurrency)
  const setToolConcurrency = useAssistantUiStore((state) => state.setToolConcurrency)
  const toolChoice = useAssistantUiStore((state) => state.toolChoice)
  const setToolChoice = useAssistantUiStore((state) => state.setToolChoice)
  const advancedPanelOpen = useAssistantUiStore((state) => state.advancedPanelOpen)
  const setAdvancedPanelOpen = useAssistantUiStore((state) => state.setAdvancedPanelOpen)
  const assistantDraft = useAgentSurfaceStore((state) => state.draft)
  const consumeAssistantDraft = useAgentSurfaceStore((state) => state.consumeDraft)
  const { t } = useTranslation()
  const locale = useAssistantLocale()
  const resolvedLanguage = getResolvedAppLanguage()
  const assistantErrorEnvelopeRenderingEnabled = useGuardrailFlag(
    GUARDRAIL_PMIDS.assistantErrorEnvelopeRendering
  )
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
    updateSessionLabel,
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

  const downloadModelMutation = api.useMutation("post", "/v1/models/download", {
    meta: {
      skipGlobalErrorToast: true,
    },
  })
  const loadModelMutation = api.useMutation("post", "/v1/models/load", {
    meta: {
      skipGlobalErrorToast: true,
    },
  })
  const switchModelMutation = api.useMutation("post", "/v1/models/switch", {
    meta: {
      skipGlobalErrorToast: true,
    },
  })
  const getTaskMutation = api.useMutation("get", "/v1/tasks/{id}", {
    meta: {
      skipGlobalErrorToast: true,
    },
  })

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
      const status = await switchModelMutation.mutateAsync({
        body: {
          model_id: modelId,
        },
      })
      setLoadedModelStatus(status)
      return
    }

    const status = await loadModelMutation.mutateAsync({
      body: {
        model_id: modelId,
      },
    })
    setLoadedModelStatus(status)
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
      setLoadedModelStatus(null)
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
        description: getAssistantErrorDescription(error, t("pages.assistant.toast.unknownError"), t, {
          preferServerEnvelope: assistantErrorEnvelopeRenderingEnabled,
        }),
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
    editAndResend,
    pendingApprovals,
    regenerateResponse,
    retryLastResponse,
    submitApproval,
  } = useAssistantAgent({
    beforeRequest: ensureAssistantModelReady,
    model: selectedModelId || "slab-llama",
    reasoningEffort,
    reasoningSupported: selectedModel?.capabilities.reasoning_controls ?? false,
    runtimePresets: selectedModel?.runtimePresets ?? null,
    sessionId: curConversation,
    systemPrompt,
    toolChoice,
    toolConcurrency,
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
    if (
      selectedModel &&
      !selectedModel.capabilities.reasoning_controls &&
      reasoningEffort !== "none"
    ) {
      setReasoningEffort("none")
    }
  }, [reasoningEffort, selectedModel, setReasoningEffort])

  const latestUserMessage = safeMessages
    .slice()
    .toReversed()
    .find((item) => item.message.role === "user")
  const currentConversationLabel =
    conversationList.find((item) => item.key === curConversation)?.label?.trim() ||
    t("pages.assistant.sessionSummary.currentSession")
  const selectedRuntimeContextLength =
    loadedModelId === selectedModelId ? loadedModelStatus?.context_length ?? null : null
  const selectedModelStatusLabel = useMemo(
    () =>
      getSelectedModelStatusLabel({
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
        selectedRuntimeContextLength,
        t,
      }),
    [
      curConversation,
      eventsConnected,
      isCreatingSession,
      isDeletingSession,
      isHistoryLoading,
      isPreparingModel,
      isSessionBootstrapping,
      modelLoading,
      resolvedLanguage,
      selectedRuntimeContextLength,
      selectedModel,
      t,
    ]
  )

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
    async (conversationKey: string, prompt: string) => {
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
        await updateSessionLabel(conversationKey, nextLabel)
      }
    },
    [conversationList, t, updateSessionLabel]
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

  const handleSelectConversation = useCallback(
    (conversationKey: string) => {
      if (conversationKey === curConversation) {
        return
      }

      if (isSessionBusy || isSessionBootstrapping) {
        toast.info(t("pages.assistant.toast.sessionSyncing"))
        return
      }

      setCurConversation(conversationKey)
    },
    [curConversation, isSessionBootstrapping, isSessionBusy, setCurConversation, t]
  )

  const handleGenerateImage = useCallback(() => {
    const prompt = draft.trim() || latestUserPrompt
    const search = prompt ? `?prompt=${encodeURIComponent(prompt)}` : ""

    navigate(
      {
        pathname: "/image",
        search,
      }
    )
  }, [draft, latestUserPrompt, navigate])

  const submitAssistantMessage = useCallback(
    async (value: string) => {
      if (!curConversation || isSessionBusy || isSessionBootstrapping) {
        toast.info(t("pages.assistant.toast.sessionSyncing"))
        return
      }

      setDraft("")
      const submitPromise = handleSubmit(value)
      void setConversationLabelIfNeeded(curConversation, value)
      await submitPromise
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

  const handleActionFeedback = useCallback((prompt: string) => {
    setDraft(prompt)
    setComposerFocusSignal((value) => value + 1)
  }, [])

  const handleSurfaceClosed = useCallback(() => {
    setComposerFocusSignal((value) => value + 1)
  }, [])

  useEffect(() => {
    if (!assistantDraft) {
      return
    }

    const draftRequest = consumeAssistantDraft()
    if (!draftRequest) {
      return
    }

    const prompt = draftRequest.prompt.trim()
    if (!prompt) {
      return
    }

    setDraft(prompt)
    setComposerFocusSignal((value) => value + 1)
    if (draftRequest.autoSubmit) {
      void submitAssistantMessage(prompt)
    }
  }, [assistantDraft, consumeAssistantDraft, submitAssistantMessage])

  const bubbleItems = useMemo(
    () => {
      const labels = {
        approve: t("pages.assistant.actions.approve"),
        assistant: t("pages.assistant.message.assistant"),
        cancelEdit: t("pages.assistant.message.cancelEdit"),
        copy: t("pages.assistant.message.copy"),
        edit: t("pages.assistant.message.edit"),
        regenerate: t("pages.assistant.message.regenerate"),
        reject: t("pages.assistant.actions.reject"),
        retry: t("pages.assistant.message.retry"),
        saveEdit: t("pages.assistant.message.saveEdit"),
        taskActionBlockedPath: t("pages.assistant.taskAction.blockedPath"),
        taskActionFeedback: t("pages.assistant.taskAction.feedback"),
        taskActionOpen: t("pages.assistant.taskAction.open"),
        taskActionReview: t("pages.assistant.taskAction.review"),
        taskActionTitle: t("pages.assistant.taskAction.title"),
        terminalCancelled: t("pages.assistant.message.cancelled"),
        thinkingLoading: t("pages.assistant.thinking.loading"),
        thinkingReady: t("pages.assistant.thinking.ready"),
        user: t("pages.assistant.message.user"),
        waitingForResponse: t("pages.assistant.message.waitingForResponse"),
      }
      const items: BubbleListProps["items"] = safeMessages.map((item) => ({
        content: {
          approvingCallIds: pendingApprovals.map((approval) => approval.callId),
          item,
          labels,
          markdownClassName: markdownThemeClassName,
          onApprove: submitApproval,
          onEdit: editAndResend,
          onFeedback: handleActionFeedback,
          onRegenerate: regenerateResponse,
          onRetry: retryLastResponse,
        },
        key: item.id,
        role: item.message.role === "assistant" ? "assistant" : "user",
        status: item.status,
      }))

      if (isPreparingModel || modelLoading) {
        items.push({
          content: (
            <span data-testid="assistant-model-loading">
              {t("pages.assistant.message.loadingModel")}
            </span>
          ),
          key: "assistant-model-loading",
          role: "system",
          status: "loading",
        })
      }

      return items
    },
    [
      isPreparingModel,
      editAndResend,
      markdownThemeClassName,
      modelLoading,
      pendingApprovals,
      regenerateResponse,
      handleActionFeedback,
      safeMessages,
      retryLastResponse,
      submitApproval,
      t,
    ]
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
              onSelectSession={handleSelectConversation}
              disableNewSession={isSessionBusy || isSessionBootstrapping}
              testIdPrefix="assistant-summary-desktop"
            />
          </div>
        </div>

        <div className="mx-auto w-full max-w-[768px] px-6 pb-6 pt-12 md:px-8 lg:px-0">
          <div className="space-y-2">
            <h1 className="text-[clamp(2.75rem,6vw,4rem)] font-semibold tracking-display text-foreground">
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
            onSelectSession={handleSelectConversation}
            disableNewSession={isSessionBusy || isSessionBootstrapping}
            testIdPrefix="assistant-summary-mobile"
          />
        </div>

        <ScrollArea className="min-h-0 flex-1">
          <div className="mx-auto flex w-full max-w-[682px] flex-col gap-8 px-6 pb-24 pt-2 md:px-8 md:pb-28 lg:px-0">
            {isSessionBootstrapping || (isHistoryLoading && safeMessages.length === 0) ? (
              <div
                className="flex min-h-[260px] items-center justify-center rounded-3xl border border-dashed border-border/60 bg-[linear-gradient(180deg,color-mix(in_oklab,var(--app-canvas)_90%,transparent)_0%,color-mix(in_oklab,var(--app-canvas)_50%,transparent)_100%)] px-8 text-center"
                data-testid="assistant-loading-state"
              >
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
              <div
                className="flex min-h-[260px] items-center justify-center rounded-3xl border border-dashed border-border/60 bg-[linear-gradient(180deg,color-mix(in_oklab,var(--app-canvas)_90%,transparent)_0%,color-mix(in_oklab,var(--app-canvas)_50%,transparent)_100%)] px-8 text-center"
                data-testid="assistant-empty-state"
              >
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
          <AgentSurfaceLayer onSurfaceClosed={handleSurfaceClosed} />
          <div className="relative mx-auto w-full max-w-[768px] px-6 pb-6 pt-4 md:px-8 lg:px-0">
            <AssistantComposer
              value={draft}
              onValueChange={setDraft}
              onSubmit={submitAssistantMessage}
              onCancel={abort}
              isRequesting={isRequesting || isPreparingModel}
              disabled={isSessionBootstrapping || isHistoryLoading || isSessionMutating || !curConversation}
              reasoningEffort={reasoningEffort}
              reasoningSupported={selectedModel?.capabilities.reasoning_controls ?? false}
              setReasoningEffort={setReasoningEffort}
              systemPrompt={systemPrompt}
              setSystemPrompt={setSystemPrompt}
              toolConcurrency={toolConcurrency}
              setToolConcurrency={setToolConcurrency}
              toolChoice={toolChoice}
              setToolChoice={setToolChoice}
              advancedPanelOpen={advancedPanelOpen}
              setAdvancedPanelOpen={setAdvancedPanelOpen}
              focusSignal={composerFocusSignal}
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

        <AssistantModelSwitchDialog
          conversationLabel={currentConversationLabel}
          isCreatingSession={isCreatingSession}
          messageCount={safeMessages.length}
          onCreateSession={() => void handleCreateSessionOnModelSwitch()}
          onKeepSession={handleKeepSessionOnModelSwitch}
          onOpenChange={(open) => {
            if (!open) {
              closePendingModelSwitch()
            }
          }}
          pendingModelId={pendingModelSwitchId}
          pendingModelLabel={pendingModelSwitch?.label}
          selectedModelLabel={selectedModel?.label}
        />
      </div>
    </XProvider>
  )
}

export default Assistant
