import XMarkdown from "@ant-design/x-markdown"
import {
  AlertCircle,
  BotMessageSquare,
  CheckCircle2,
  ClipboardCheck,
  Hammer,
  Loader2,
  Plus,
  SendHorizontal,
  ShieldCheck,
  Square,
  UserRound,
  XCircle,
} from "lucide-react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { toast } from "sonner"

import "@ant-design/x-markdown/themes/dark.css"
import "@ant-design/x-markdown/themes/light.css"

import api, { getErrorMessage } from "@slab/api"
import { SERVER_BASE_URL } from "@slab/api/config"
import { toCatalogModelList } from "@slab/api/models"
import type { components } from "@slab/api/v1"
import { Badge } from "@slab/components/badge"
import { Button } from "@slab/components/button"
import { ScrollArea } from "@slab/components/scroll-area"
import { Textarea } from "@slab/components/textarea"
import { StatusPill } from "@slab/components/workspace"
import { useTranslation } from "@slab/i18n"

import { usePageHeader, usePageHeaderControl } from "@/hooks/use-global-header-meta"
import { usePersistedHeaderSelect } from "@/hooks/use-persisted-header-select"
import { PAGE_HEADER_META } from "@/layouts/header-meta"
import { HEADER_SELECT_KEYS } from "@/layouts/header-controls"
import { cn } from "@/lib/utils"

type AgentStatus = components["schemas"]["AgentStatusValue"]

type AgentMessage = {
  id: string
  role: "assistant" | "user"
  content: string
  status?: "loading" | "complete" | "error"
}

type AgentTimelineItem = {
  id: string
  type: "approval" | "error" | "output" | "status" | "tool"
  title: string
  detail?: string
}

type PendingApproval = {
  callId: string
  toolName: string
  command: string
}

type AgentStreamEvent =
  | { type: "agent_status"; status: AgentStatus }
  | { type: "approval_required"; call_id: string; tool_name: string; command: string }
  | { type: "assistant_delta"; text: string }
  | { type: "lagged" }
  | { type: "tool_call_output"; call_id: string; output: string }
  | { type: "tool_call_started"; tool_name: string; call_id: string; arguments: string }
  | { type: "turn_completed"; text: string }
  | { type: "turn_failed"; error: string }

type AgentModelOption = {
  id: string
  label: string
  disabled?: boolean
}

function nextId(prefix: string) {
  const random =
    typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
      ? crypto.randomUUID()
      : `${Date.now()}-${Math.random().toString(36).slice(2)}`

  return `${prefix}-${random}`
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null
}

function parseAgentStreamEvent(data: string): AgentStreamEvent | null {
  let value: unknown
  try {
    value = JSON.parse(data)
  } catch {
    return null
  }

  if (!isRecord(value) || typeof value.type !== "string") {
    return null
  }

  switch (value.type) {
    case "agent_status":
      return typeof value.status === "string"
        ? { type: "agent_status", status: value.status as AgentStatus }
        : null
    case "approval_required":
      return typeof value.call_id === "string" &&
        typeof value.tool_name === "string" &&
        typeof value.command === "string"
        ? {
            type: "approval_required",
            call_id: value.call_id,
            tool_name: value.tool_name,
            command: value.command,
          }
        : null
    case "assistant_delta":
      return typeof value.text === "string"
        ? { type: "assistant_delta", text: value.text }
        : null
    case "lagged":
      return { type: "lagged" }
    case "tool_call_output":
      return typeof value.call_id === "string" && typeof value.output === "string"
        ? { type: "tool_call_output", call_id: value.call_id, output: value.output }
        : null
    case "tool_call_started":
      return typeof value.tool_name === "string" &&
        typeof value.call_id === "string" &&
        typeof value.arguments === "string"
        ? {
            type: "tool_call_started",
            tool_name: value.tool_name,
            call_id: value.call_id,
            arguments: value.arguments,
          }
        : null
    case "turn_completed":
      return typeof value.text === "string" ? { type: "turn_completed", text: value.text } : null
    case "turn_failed":
      return typeof value.error === "string" ? { type: "turn_failed", error: value.error } : null
    default:
      return null
  }
}

function isBusyStatus(status: AgentStatus | null) {
  return status === "pending" || status === "running"
}

function statusTone(status: AgentStatus | null): "danger" | "info" | "neutral" | "success" {
  switch (status) {
    case "completed":
      return "success"
    case "errored":
      return "danger"
    case "pending":
    case "running":
      return "info"
    case "shutdown":
    case null:
      return "neutral"
  }
}

function timelineIcon(type: AgentTimelineItem["type"]) {
  switch (type) {
    case "approval":
      return ShieldCheck
    case "error":
      return AlertCircle
    case "output":
      return ClipboardCheck
    case "status":
      return CheckCircle2
    case "tool":
      return Hammer
  }
}

function Agent() {
  const { t } = useTranslation()
  const [threadId, setThreadId] = useState<string | null>(null)
  const [status, setStatus] = useState<AgentStatus | null>(null)
  const [draft, setDraft] = useState("")
  const [messages, setMessages] = useState<AgentMessage[]>([])
  const [timeline, setTimeline] = useState<AgentTimelineItem[]>([])
  const [pendingApproval, setPendingApproval] = useState<PendingApproval | null>(null)
  const [eventsConnected, setEventsConnected] = useState(false)
  const bottomRef = useRef<HTMLDivElement | null>(null)
  const eventSourceRef = useRef<EventSource | null>(null)
  const seenEventIdsRef = useRef<Set<string>>(new Set())

  const {
    data: catalogModels,
    isLoading: catalogModelsLoading,
  } = api.useQuery("get", "/v1/models", {
    params: {
      query: {
        capability: "chat_generation",
      },
    },
  })
  const spawnMutation = api.useMutation("post", "/v1/agents/spawn")
  const inputMutation = api.useMutation("post", "/v1/agents/{id}/input")
  const approveMutation = api.useMutation("post", "/v1/agents/{id}/approve")
  const interruptMutation = api.useMutation("post", "/v1/agents/{id}/interrupt")

  const parsedCatalogModels = useMemo(() => toCatalogModelList(catalogModels), [catalogModels])
  const modelOptions = useMemo<AgentModelOption[]>(
    () =>
      parsedCatalogModels.map((model) => {
        const ready =
          model.kind === "cloud" ||
          (model.status === "ready" &&
            typeof model.local_path === "string" &&
            model.local_path.length > 0)

        return {
          id: model.id,
          label: model.display_name,
          disabled: !ready,
        }
      }),
    [parsedCatalogModels]
  )
  const { value: selectedModelId, setValue: setSelectedModelId } = usePersistedHeaderSelect({
    key: HEADER_SELECT_KEYS.agentModel,
    options: modelOptions,
    isLoading: catalogModelsLoading,
  })
  const selectedModel = useMemo(
    () => modelOptions.find((model) => model.id === selectedModelId),
    [modelOptions, selectedModelId]
  )
  const isRequesting =
    isBusyStatus(status) ||
    spawnMutation.isPending ||
    inputMutation.isPending ||
    approveMutation.isPending
  const headerModelPicker = useMemo(
    () => ({
      type: "select" as const,
      value: selectedModelId,
      options: modelOptions,
      onValueChange: setSelectedModelId,
      groupLabel: t("pages.agent.modelPicker.groupLabel"),
      placeholder: t("pages.agent.modelPicker.placeholder"),
      loading: catalogModelsLoading,
      disabled: isRequesting || modelOptions.length === 0,
      emptyLabel: t("pages.agent.modelPicker.emptyLabel"),
    }),
    [
      catalogModelsLoading,
      isRequesting,
      modelOptions,
      selectedModelId,
      setSelectedModelId,
      t,
    ]
  )

  usePageHeader({
    ...PAGE_HEADER_META.agent,
    title: t("pages.agent.header.title"),
    subtitle: t("pages.agent.header.subtitle"),
  })
  usePageHeaderControl(headerModelPicker)

  const addTimelineEvent = useCallback(
    (type: AgentTimelineItem["type"], title: string, detail?: string) => {
      setTimeline((current) => [
        ...current.slice(-80),
        { id: nextId("event"), type, title, detail },
      ])
    },
    []
  )

  const appendUserMessage = useCallback((content: string) => {
    setMessages((current) => [
      ...current,
      { id: nextId("user"), role: "user", content, status: "complete" },
    ])
  }, [])

  const appendAssistantDelta = useCallback((text: string) => {
    setMessages((current) => {
      for (let index = current.length - 1; index >= 0; index -= 1) {
        const message = current[index]
        if (message?.role === "assistant" && message.status === "loading") {
          const next = [...current]
          next[index] = { ...message, content: `${message.content}${text}` }
          return next
        }
      }

      return [
        ...current,
        { id: nextId("assistant"), role: "assistant", content: text, status: "loading" },
      ]
    })
  }, [])

  const completeAssistantTurn = useCallback((text: string) => {
    setMessages((current) => {
      for (let index = current.length - 1; index >= 0; index -= 1) {
        const message = current[index]
        if (message?.role === "assistant" && message.status === "loading") {
          const next = [...current]
          next[index] = {
            ...message,
            content: text || message.content,
            status: "complete",
          }
          return next
        }
      }

      if (!text.trim()) {
        return current
      }

      return [
        ...current,
        { id: nextId("assistant"), role: "assistant", content: text, status: "complete" },
      ]
    })
  }, [])

  const appendAssistantError = useCallback((content: string) => {
    setMessages((current) => [
      ...current,
      { id: nextId("assistant"), role: "assistant", content, status: "error" },
    ])
  }, [])

  const handleAgentEvent = useCallback(
    (data: string) => {
      const event = parseAgentStreamEvent(data)
      if (!event) {
        return
      }

      switch (event.type) {
        case "agent_status":
          setStatus(event.status)
          addTimelineEvent("status", t(`pages.agent.status.${event.status}`))
          break
        case "approval_required":
          setPendingApproval({
            callId: event.call_id,
            toolName: event.tool_name,
            command: event.command,
          })
          addTimelineEvent(
            "approval",
            t("pages.agent.timeline.approvalRequired", { tool: event.tool_name }),
            event.command
          )
          break
        case "assistant_delta":
          appendAssistantDelta(event.text)
          break
        case "lagged":
          addTimelineEvent("error", t("pages.agent.timeline.lagged"))
          break
        case "tool_call_output":
          addTimelineEvent("output", t("pages.agent.timeline.toolOutput"), event.output)
          break
        case "tool_call_started":
          addTimelineEvent(
            "tool",
            t("pages.agent.timeline.toolStarted", { tool: event.tool_name }),
            event.arguments
          )
          break
        case "turn_completed":
          setStatus("completed")
          setPendingApproval(null)
          completeAssistantTurn(event.text)
          addTimelineEvent("status", t("pages.agent.timeline.turnCompleted"))
          break
        case "turn_failed":
          setStatus("errored")
          setPendingApproval(null)
          appendAssistantError(event.error)
          addTimelineEvent("error", t("pages.agent.timeline.turnFailed"), event.error)
          break
      }
    },
    [addTimelineEvent, appendAssistantDelta, appendAssistantError, completeAssistantTurn, t]
  )

  useEffect(() => {
    seenEventIdsRef.current.clear()
  }, [threadId])

  useEffect(() => {
    if (!threadId) {
      setEventsConnected(false)
      return undefined
    }

    const source = new EventSource(
      `${SERVER_BASE_URL}/v1/agents/${encodeURIComponent(threadId)}/events`
    )
    const handleOpen = () => setEventsConnected(true)
    const handleError = () => setEventsConnected(false)
    const handleMessage = (message: MessageEvent<string>) => {
      const eventId = message.lastEventId || message.data
      if (seenEventIdsRef.current.has(eventId)) {
        return
      }
      seenEventIdsRef.current.add(eventId)
      handleAgentEvent(message.data)
    }
    eventSourceRef.current = source
    source.addEventListener("open", handleOpen)
    source.addEventListener("error", handleError)
    source.addEventListener("message", handleMessage)

    return () => {
      source.removeEventListener("open", handleOpen)
      source.removeEventListener("error", handleError)
      source.removeEventListener("message", handleMessage)
      source.close()
      if (eventSourceRef.current === source) {
        eventSourceRef.current = null
      }
      setEventsConnected(false)
    }
  }, [handleAgentEvent, threadId])

  useEffect(() => {
    bottomRef.current?.scrollIntoView({
      behavior: messages.length > 0 ? "smooth" : "auto",
      block: "end",
    })
  }, [isRequesting, messages])

  const submitPrompt = useCallback(async () => {
    const prompt = draft.trim()
    if (!prompt || isRequesting) {
      return
    }

    if (!selectedModelId || selectedModel?.disabled) {
      toast.error(t("pages.agent.toast.selectModel"))
      return
    }

    setDraft("")
    appendUserMessage(prompt)
    setStatus("pending")
    setPendingApproval(null)

    try {
      if (!threadId) {
        const response = await spawnMutation.mutateAsync({
          body: {
            session_id: nextId("session"),
            config: {
              model: selectedModelId,
              max_turns: 8,
            },
            messages: [{ role: "user", content: prompt }],
          },
        })
        setThreadId(response.thread_id)
      } else {
        await inputMutation.mutateAsync({
          params: { path: { id: threadId } },
          body: { content: prompt },
        })
        setStatus("running")
      }
    } catch (error) {
      const message = getErrorMessage(error)
      setStatus("errored")
      appendAssistantError(message)
      toast.error(t("pages.agent.toast.requestFailed"), { description: message })
    }
  }, [
    appendAssistantError,
    appendUserMessage,
    draft,
    inputMutation,
    isRequesting,
    selectedModel,
    selectedModelId,
    spawnMutation,
    t,
    threadId,
  ])

  const submitApproval = useCallback(
    async (approved: boolean) => {
      if (!threadId || !pendingApproval) {
        return
      }

      const decision = pendingApproval
      setPendingApproval(null)
      addTimelineEvent(
        approved ? "status" : "error",
        approved ? t("pages.agent.timeline.approved") : t("pages.agent.timeline.rejected"),
        decision.command
      )

      try {
        const response = await approveMutation.mutateAsync({
          params: { path: { id: threadId } },
          body: { call_id: decision.callId, approved },
        })

        if (!response.delivered) {
          toast.error(t("pages.agent.toast.approvalNotDelivered"))
        }
      } catch (error) {
        const message = getErrorMessage(error)
        toast.error(t("pages.agent.toast.approvalFailed"), { description: message })
      }
    },
    [addTimelineEvent, approveMutation, pendingApproval, t, threadId]
  )

  const interruptThread = useCallback(async () => {
    if (!threadId || !isRequesting) {
      return
    }

    try {
      await interruptMutation.mutateAsync({
        params: { path: { id: threadId } },
      })
      setStatus("shutdown")
      addTimelineEvent("status", t("pages.agent.timeline.interrupted"))
    } catch (error) {
      const message = getErrorMessage(error)
      toast.error(t("pages.agent.toast.interruptFailed"), { description: message })
    }
  }, [addTimelineEvent, interruptMutation, isRequesting, t, threadId])

  const startNewThread = useCallback(() => {
    eventSourceRef.current?.close()
    eventSourceRef.current = null
    seenEventIdsRef.current.clear()
    setThreadId(null)
    setStatus(null)
    setDraft("")
    setMessages([])
    setTimeline([])
    setPendingApproval(null)
    setEventsConnected(false)
  }, [])

  const statusLabel = status ? t(`pages.agent.status.${status}`) : t("pages.agent.status.idle")
  const connectionLabel = eventsConnected
    ? t("pages.agent.connection.connected")
    : threadId
      ? t("pages.agent.connection.reconnecting")
      : t("pages.agent.connection.idle")

  return (
    <div className="flex min-h-0 w-full flex-1 flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <StatusPill status={statusTone(status)}>{statusLabel}</StatusPill>
            <Badge variant="chip">{connectionLabel}</Badge>
            {threadId ? (
              <Badge variant="chip" className="max-w-[18rem] truncate">
                {threadId}
              </Badge>
            ) : null}
          </div>
          <p className="mt-2 truncate text-sm text-muted-foreground">
            {selectedModel?.label ?? t("pages.agent.modelPicker.placeholder")}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="pill" size="pill" onClick={startNewThread}>
            <Plus className="size-4" />
            {t("pages.agent.actions.newThread")}
          </Button>
          <Button
            variant="quiet"
            size="icon"
            disabled={!isRequesting || !threadId}
            onClick={() => void interruptThread()}
            aria-label={t("pages.agent.actions.interrupt")}
            title={t("pages.agent.actions.interrupt")}
          >
            <Square className="size-4" />
          </Button>
        </div>
      </div>

      <div className="grid min-h-0 flex-1 gap-4 xl:grid-cols-[minmax(0,1fr)_22rem]">
        <section className="workspace-surface flex min-h-0 flex-col overflow-hidden rounded-[28px]">
          <ScrollArea className="min-h-0 flex-1">
            <div className="mx-auto flex w-full max-w-[760px] flex-col gap-6 px-5 py-6">
              {messages.length === 0 ? (
                <div className="flex min-h-[280px] items-center justify-center rounded-[24px] border border-dashed border-border/60 px-8 text-center">
                  <div className="max-w-md space-y-3">
                    <BotMessageSquare className="mx-auto size-9 text-muted-foreground" />
                    <p className="text-base font-medium text-foreground">
                      {t("pages.agent.empty.title")}
                    </p>
                    <p className="text-sm leading-6 text-muted-foreground">
                      {t("pages.agent.empty.description")}
                    </p>
                  </div>
                </div>
              ) : (
                messages.map((message) => (
                  <AgentMessageBubble key={message.id} message={message} />
                ))
              )}
              <div ref={bottomRef} />
            </div>
          </ScrollArea>

          <div className="shrink-0 border-t border-border/60 bg-[var(--shell-card)] px-5 py-4">
            <div className="mx-auto flex w-full max-w-[760px] items-end gap-3 rounded-[24px] bg-[var(--surface-input)] p-2 shadow-[var(--shell-elevation)]">
              <Textarea
                value={draft}
                variant="shell"
                autoResize
                disabled={isRequesting}
                onChange={(event) => setDraft(event.target.value)}
                placeholder={t("pages.agent.composer.placeholder")}
                className="min-h-[48px] max-h-48 resize-none border-0 bg-transparent px-3 py-3 text-base text-foreground shadow-none placeholder:text-muted-foreground/60 focus-visible:ring-0"
                onKeyDown={(event) => {
                  if (event.key === "Enter" && !event.shiftKey) {
                    event.preventDefault()
                    void submitPrompt()
                  }
                }}
              />
              <Button
                variant="cta"
                size="icon"
                className="mb-1 size-10 rounded-full"
                disabled={isRequesting || !draft.trim() || !selectedModelId || selectedModel?.disabled}
                onClick={() => void submitPrompt()}
                aria-label={t("pages.agent.composer.send")}
              >
                {isRequesting ? (
                  <Loader2 className="size-4 animate-spin" />
                ) : (
                  <SendHorizontal className="size-4" />
                )}
              </Button>
            </div>
          </div>
        </section>

        <aside className="workspace-soft-panel flex min-h-0 flex-col overflow-hidden rounded-[28px] p-0">
          <div className="border-b border-border/60 px-4 py-3">
            <p className="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
              {t("pages.agent.timeline.title")}
            </p>
          </div>

          {pendingApproval ? (
            <div className="border-b border-border/60 p-4">
              <div className="rounded-[20px] border border-[color-mix(in_oklab,var(--brand-gold)_35%,var(--border))] bg-[color-mix(in_oklab,var(--brand-gold)_8%,var(--surface-1))] p-4">
                <div className="flex items-center gap-2">
                  <ShieldCheck className="size-4 text-[var(--brand-gold)]" />
                  <p className="text-sm font-semibold text-foreground">
                    {pendingApproval.toolName}
                  </p>
                </div>
                <pre className="mt-3 max-h-36 overflow-auto whitespace-pre-wrap break-words rounded-[14px] bg-background/60 p-3 text-xs leading-5 text-muted-foreground">
                  {pendingApproval.command}
                </pre>
                <div className="mt-3 flex items-center justify-end gap-2">
                  <Button
                    variant="quiet"
                    size="sm"
                    onClick={() => void submitApproval(false)}
                    disabled={approveMutation.isPending}
                  >
                    <XCircle className="size-4" />
                    {t("pages.agent.actions.reject")}
                  </Button>
                  <Button
                    variant="pill"
                    size="sm"
                    onClick={() => void submitApproval(true)}
                    disabled={approveMutation.isPending}
                  >
                    <CheckCircle2 className="size-4" />
                    {t("pages.agent.actions.approve")}
                  </Button>
                </div>
              </div>
            </div>
          ) : null}

          <ScrollArea className="min-h-0 flex-1">
            <div className="space-y-3 p-4">
              {timeline.length === 0 ? (
                <p className="rounded-[18px] bg-[var(--surface-1)] px-4 py-3 text-sm text-muted-foreground">
                  {t("pages.agent.timeline.empty")}
                </p>
              ) : (
                timeline.map((item) => <TimelineItem key={item.id} item={item} />)
              )}
            </div>
          </ScrollArea>
        </aside>
      </div>
    </div>
  )
}

function AgentMessageBubble({ message }: { message: AgentMessage }) {
  const { t } = useTranslation()
  const isAssistant = message.role === "assistant"
  const isLoading = message.status === "loading"
  const isError = message.status === "error"

  return (
    <div className={cn("flex w-full", isAssistant ? "justify-start" : "justify-end")}>
      <div className={cn("flex max-w-[min(100%,42rem)] flex-col gap-2", !isAssistant && "items-end")}>
        <div className={cn("flex items-center gap-2 px-1", !isAssistant && "flex-row-reverse")}>
          <span
            className={cn(
              "flex size-6 shrink-0 items-center justify-center",
              isAssistant
                ? "rounded-[8px] bg-[var(--brand-teal)] text-[var(--brand-teal-foreground)]"
                : "rounded-full border border-border/30 bg-[var(--shell-card)] text-foreground/70"
            )}
          >
            {isAssistant ? (
              <BotMessageSquare className="size-3.5" />
            ) : (
              <UserRound className="size-3.5" />
            )}
          </span>
          <span className="text-[11px] font-bold uppercase tracking-[0.14em] text-muted-foreground">
            {isAssistant ? t("pages.agent.message.agent") : t("pages.agent.message.user")}
          </span>
        </div>

        <div
          className={cn(
            "px-6 py-4 shadow-[var(--shell-elevation)]",
            isAssistant
              ? "rounded-tl-[24px] rounded-tr-[24px] rounded-br-[24px] rounded-bl-[8px] bg-[var(--ai-bubble)] text-[var(--ai-bubble-foreground)]"
              : "rounded-tl-[24px] rounded-tr-[24px] rounded-br-[8px] rounded-bl-[24px] bg-[var(--user-bubble)] text-[var(--user-bubble-foreground)]",
            isError && "border border-destructive/35"
          )}
        >
          {isAssistant ? (
            <XMarkdown
              paragraphTag="div"
              className="chat-markdown chat-markdown--assistant text-base leading-[1.625]"
              streaming={{
                hasNextChunk: isLoading,
                enableAnimation: true,
              }}
            >
              {message.content || t("pages.agent.message.waiting")}
            </XMarkdown>
          ) : (
            <p className="whitespace-pre-wrap text-base leading-[1.625]">{message.content}</p>
          )}
        </div>
      </div>
    </div>
  )
}

function TimelineItem({ item }: { item: AgentTimelineItem }) {
  const Icon = timelineIcon(item.type)

  return (
    <div className="rounded-[18px] bg-[var(--surface-1)] px-4 py-3">
      <div className="flex items-start gap-3">
        <span className="mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-[10px] bg-[var(--surface-soft)] text-muted-foreground">
          <Icon className="size-3.5" />
        </span>
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium text-foreground">{item.title}</p>
          {item.detail ? (
            <pre className="mt-2 max-h-32 overflow-auto whitespace-pre-wrap break-words text-xs leading-5 text-muted-foreground">
              {item.detail}
            </pre>
          ) : null}
        </div>
      </div>
    </div>
  )
}

export default Agent
