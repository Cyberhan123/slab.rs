import {
  Bubble,
  CodeHighlighter,
  ThoughtChain,
  XProvider,
  type BubbleListProps,
  type ThoughtChainItemType,
} from "@ant-design/x"
import { BotMessageSquare, CheckCircle2, Copy, Loader2, UserRound, XCircle } from "lucide-react"
import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react"
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
import api, { type components } from "@slab/api"
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
  stripTrailingAssistantTurnArtifacts,
  type AssistantMessageRecord,
  type AssistantThought,
} from "./assistant-context"
import { AssistantComposer } from "./components/assistant-composer"
import { AssistantMarkdown } from "./components/assistant-markdown"
import { AssistantSessionSheet } from "./components/assistant-session-sheet"
import {
  AssistantSessionSummaryCard,
  type AssistantSessionSummaryItem,
} from "./components/assistant-session-summary-card"
import { useAssistantLocale } from "./assistant-locale"
import { useAssistantAgent } from "./hooks/use-assistant-agent"
import { useAssistantSessions } from "./hooks/use-assistant-sessions"
import { useMarkdownTheme } from "./hooks/use-markdown-theme"
import { cn } from "@/lib/utils"

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

type ModelRuntimeStatus = components["schemas"]["ModelStatusResponse"]

type AssistantBubbleContent = {
  approving: boolean
  item: AssistantMessageRecord
  labels: {
    approve: string
    assistant: string
    copy: string
    reject: string
    thinkingLoading: string
    thinkingReady: string
    user: string
    waitingForResponse: string
  }
  markdownClassName?: string
  onApprove?: (approved: boolean) => void
}

type ParsedThinkingContent = {
  thinking: string | null
  answer: string
  thinkingLoading: boolean
}

function parseThinkingContent(rawContent: string): ParsedThinkingContent {
  const openTagIndex = rawContent.indexOf("<think")
  if (openTagIndex < 0) {
    return { thinking: null, answer: rawContent, thinkingLoading: false }
  }

  const openTagEnd = rawContent.indexOf(">", openTagIndex)
  if (openTagEnd < 0) {
    return { thinking: null, answer: rawContent, thinkingLoading: false }
  }

  const openTag = rawContent.slice(openTagIndex, openTagEnd + 1)
  const thinkingMarkedDone = /\bstatus\s*=\s*["']?done["']?/i.test(openTag)
  const closeTag = "</think>"
  const closeTagIndex = rawContent.indexOf(closeTag, openTagEnd + 1)

  if (closeTagIndex < 0) {
    const thinking = rawContent.slice(openTagEnd + 1).trimStart()

    return {
      answer: rawContent.slice(0, openTagIndex).trimEnd(),
      thinking: thinking || null,
      thinkingLoading: !thinkingMarkedDone,
    }
  }

  const thinking = rawContent.slice(openTagEnd + 1, closeTagIndex).trim()
  const before = rawContent.slice(0, openTagIndex)
  const after = rawContent.slice(closeTagIndex + closeTag.length)

  return {
    answer: `${before}${after}`.trimStart(),
    thinking: thinking || null,
    thinkingLoading: false,
  }
}

function guessCodeLanguage(value: string) {
  const trimmed = value.trim()
  if (trimmed.startsWith("diff --git") || trimmed.startsWith("--- ") || trimmed.startsWith("*** ")) {
    return "diff"
  }

  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    return "json"
  }

  return "text"
}

function renderThoughtContent(
  thought: AssistantThought,
  approving: boolean,
  onApprove: ((approved: boolean) => void) | undefined,
  labels: {
    approve: string
    reject: string
  }
) {
  if (thought.pendingApproval) {
    return (
      <div className="space-y-3">
        <CodeHighlighter lang="shell" className="rounded-[14px] border border-border/60 text-xs">
          {thought.pendingApproval.command}
        </CodeHighlighter>
        <div className="flex justify-end gap-2">
          <Button
            variant="quiet"
            size="sm"
            onClick={() => onApprove?.(false)}
            disabled={approving}
          >
            <XCircle className="size-4" />
            {labels.reject}
          </Button>
          <Button
            variant="pill"
            size="sm"
            onClick={() => onApprove?.(true)}
            disabled={approving}
          >
            <CheckCircle2 className="size-4" />
            {labels.approve}
          </Button>
        </div>
      </div>
    )
  }

  return thought.detail ? (
    <CodeHighlighter
      lang={guessCodeLanguage(thought.detail)}
      className="rounded-[14px] border border-border/60 text-xs"
    >
      {thought.detail}
    </CodeHighlighter>
  ) : null
}

function toThoughtChainItems(
  thoughts: AssistantThought[] | undefined,
  approving: boolean,
  onApprove: ((approved: boolean) => void) | undefined,
  labels: {
    approve: string
    reject: string
  },
  thinking?: {
    content: string
    key: string
    loading: boolean
    title: string
  }
): ThoughtChainItemType[] {
  const items = (thoughts ?? []).map((thought) => ({
    blink: thought.status === "loading",
    collapsible: true,
    content: renderThoughtContent(thought, approving, onApprove, labels),
    description: thought.summary ?? thought.toolName ?? thought.callId,
    icon: false,
    key: thought.id,
    status: thought.status,
    title: thought.title,
  }))

  if (!thinking?.content) {
    return items
  }

  return [
    {
      blink: thinking.loading,
      collapsible: true,
      content: (
        <AssistantMarkdown className="assistant-markdown--assistant" hasNextChunk={thinking.loading}>
          {thinking.content}
        </AssistantMarkdown>
      ),
      icon: false,
      key: thinking.key,
      status: thinking.loading ? "loading" : "success",
      title: thinking.title,
    },
    ...items,
  ]
}

const AssistantBubbleContentView = memo(function AssistantBubbleContentView({
  content,
}: {
  content: AssistantBubbleContent
}) {
  const role = content.item.message.role
  const isAssistant = role === "assistant"
  const isBusy = content.item.status === "loading" || content.item.status === "updating"
  const hasNextChunk = content.item.status === "updating"
  const rawContent = stripTrailingAssistantTurnArtifacts(
    getAssistantMessageTextContent(content.item.message)
  )
  const parsed = useMemo(() => parseThinkingContent(rawContent), [rawContent])
  const liveThinking =
    typeof content.item.message.reasoningContent === "string"
      ? content.item.message.reasoningContent.trim()
      : ""
  const thinking = liveThinking || parsed.thinking
  const answer = liveThinking
    ? rawContent.includes("<think")
      ? parsed.answer
      : rawContent
    : parsed.answer
  const thinkingLoading = liveThinking ? isBusy : parsed.thinkingLoading
  const thoughtItems = useMemo(
    () =>
      toThoughtChainItems(
        content.item.message.thoughts,
        content.approving,
        content.onApprove,
        {
          approve: content.labels.approve,
          reject: content.labels.reject,
        },
        thinking
          ? {
              content: thinking,
              key: `${content.item.id}-thinking`,
              loading: thinkingLoading && isBusy,
              title: thinkingLoading && isBusy ? content.labels.thinkingLoading : content.labels.thinkingReady,
            }
          : undefined
      ),
    [
      content.approving,
      content.item.id,
      content.item.message.thoughts,
      content.labels.approve,
      content.labels.reject,
      content.labels.thinkingLoading,
      content.labels.thinkingReady,
      content.onApprove,
      isBusy,
      thinking,
      thinkingLoading,
    ]
  )
  const expandedThoughtKeys = useMemo(
    () => thoughtItems.map((thought) => thought.key).filter((key): key is string => Boolean(key)),
    [thoughtItems]
  )

  return (
    <div className="space-y-4">
      {isAssistant && thoughtItems.length > 0 ? (
        <ThoughtChain
          items={thoughtItems}
          defaultExpandedKeys={expandedThoughtKeys}
          className="rounded-[18px] border border-border/50 bg-background/30 px-4 py-3"
        />
      ) : null}

      {answer ? (
        <AssistantMarkdown
          className={cn(
            isAssistant ? "assistant-markdown--assistant" : "assistant-markdown--user",
            content.markdownClassName
          )}
          hasNextChunk={hasNextChunk}
        >
          {answer}
        </AssistantMarkdown>
      ) : isBusy ? (
        <p className="text-sm opacity-80">{content.labels.waitingForResponse}</p>
      ) : null}
    </div>
  )
})

function renderAssistantBubbleContent(content: AssistantBubbleContent) {
  return <AssistantBubbleContentView content={content} />
}

const ASSISTANT_BUBBLE_ROLES = {
  assistant: {
    avatar: (
      <span className="flex size-6 shrink-0 items-center justify-center rounded-[8px] bg-[var(--brand-teal)] text-[var(--brand-teal-foreground)]">
        <BotMessageSquare className="size-3.5" />
      </span>
    ),
    contentRender: renderAssistantBubbleContent,
    footer: (content: AssistantBubbleContent) => (
      <Button
        type="button"
        variant="quiet"
        size="sm"
        className="h-7 rounded-full px-3 text-[11px] text-muted-foreground hover:text-foreground"
        onClick={() => void navigator.clipboard.writeText(getAssistantMessageTextContent(content.item.message))}
      >
        <Copy className="size-3.5" />
        {content.labels.copy}
      </Button>
    ),
    header: (content: AssistantBubbleContent) => content.labels.assistant,
    placement: "start",
    shape: "corner",
    styles: {
      content: {
        background: "var(--ai-bubble)",
        color: "var(--ai-bubble-foreground)",
      },
    },
    variant: "filled",
  },
  user: {
    avatar: (
      <span className="flex size-6 shrink-0 items-center justify-center rounded-full border border-border/30 bg-[var(--shell-card)] text-foreground/70">
        <UserRound className="size-3.5" />
      </span>
    ),
    contentRender: renderAssistantBubbleContent,
    footer: (content: AssistantBubbleContent) => (
      <Button
        type="button"
        variant="quiet"
        size="sm"
        className="h-7 rounded-full px-3 text-[11px] text-muted-foreground hover:text-foreground"
        onClick={() => void navigator.clipboard.writeText(getAssistantMessageTextContent(content.item.message))}
      >
        <Copy className="size-3.5" />
        {content.labels.copy}
      </Button>
    ),
    header: (content: AssistantBubbleContent) => content.labels.user,
    placement: "end",
    shape: "corner",
    styles: {
      content: {
        background: "var(--user-bubble)",
        color: "var(--user-bubble-foreground)",
      },
    },
    variant: "filled",
  },
  system: {
    shape: "round",
    variant: "outlined",
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
  const [loadedModelStatus, setLoadedModelStatus] = useState<ModelRuntimeStatus | null>(null)
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
  const selectedRuntimeContextLength =
    loadedModelId === selectedModelId ? loadedModelStatus?.context_length ?? null : null
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

    if (selectedRuntimeContextLength && selectedRuntimeContextLength > 0) {
      parts.push(
        t("pages.assistant.status.runtimeContextWindow", {
          formatted: new Intl.NumberFormat(resolvedLanguage).format(selectedRuntimeContextLength),
        })
      )
    } else if (selectedModel.contextWindow && selectedModel.contextWindow > 0) {
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
    selectedRuntimeContextLength,
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

      setDraft("")
      await setConversationLabelIfNeeded(curConversation, value)
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
    () => {
      const labels = {
        approve: t("pages.assistant.actions.approve"),
        assistant: t("pages.assistant.message.assistant"),
        copy: t("pages.assistant.message.copy"),
        reject: t("pages.assistant.actions.reject"),
        thinkingLoading: t("pages.assistant.thinking.loading"),
        thinkingReady: t("pages.assistant.thinking.ready"),
        user: t("pages.assistant.message.user"),
        waitingForResponse: t("pages.assistant.message.waitingForResponse"),
      }
      const items: BubbleListProps["items"] = safeMessages.map((item) => ({
        content: {
          approving: isRequesting,
          item,
          labels,
          markdownClassName: markdownThemeClassName,
          onApprove: submitApproval,
        },
        key: item.id,
        role: item.message.role === "assistant" ? "assistant" : "user",
        status: item.status,
      }))

      if (isPreparingModel || modelLoading) {
        items.push({
          content: selectedModelStatusLabel,
          key: "assistant-model-loading",
          role: "system",
          status: "loading",
        })
      }

      return items
    },
    [
      isPreparingModel,
      isRequesting,
      markdownThemeClassName,
      modelLoading,
      safeMessages,
      selectedModelStatusLabel,
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
            onSelectSession={handleSelectConversation}
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
