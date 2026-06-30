import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import { translateServerField, useTranslation } from '@slab/i18n'

import api, { createSlabApiFetchClient, getErrorMessage } from '@slab/api'
import { GUARDRAIL_PMIDS, useGuardrailFlag } from '@/lib/guardrail-flags'
import { useAgentSurfaceStore } from '@/store/useAgentSurfaceStore'

import {
  DEFAULT_CONVERSATION_KEY,
  getAssistantMessageTextContent,
  isEphemeralConversationKey,
  stripTrailingAssistantTurnArtifacts,
  toAssistantRequestMessages,
  type AssistantArtifactRef,
  type AgentResponsesClientMessage,
  type AgentResponsesServerMessage,
  type AgentStatus,
  type AssistantMessageRecord,
  type AssistantRuntimePresets,
  type AssistantThought,
} from '../assistant-context'
import { useAssistantLocale } from '../assistant-locale'
import {
  parseAssistantAgentServerMessage,
  parseAssistantAgentStreamEvent,
  type AssistantAgentStreamEvent,
} from '../lib/assistant-agent-events'
import {
  isAbortError,
  nextReconnectDelayMs,
  readAssistantSseStream,
} from '../lib/assistant-sse'
import {
  agentEventKey,
  agentResponsesSseUrl,
  agentResponsesWebSocketUrl,
  isBusyStatus,
  nextId,
  parseAssistantSlashCommand,
  serverMessageThreadId,
  toAgentConfig,
  updateLastAssistantMessage,
  withThoughts,
  type AssistantReasoningEffort,
  type AssistantToolChoice,
} from '../lib/assistant-agent-state'
import { withAssistantMessageReasoningContent } from '../lib/assistant-message-utils'
import {
  formatKnownToolResult,
  projectAgentThreadMessages,
} from '../lib/assistant-message-projection'
import { dispatchA2uToolCall } from '../lib/a2u-dispatcher'
import { parsePlanProgress, type PlanProgress } from '../lib/plan-progress'

type PendingApproval = {
  callId: string
  toolName: string
  command: string
}

const MAX_SSE_RECONNECT_ATTEMPTS = 6

function shouldIgnoreAfterAbort(event: AssistantAgentStreamEvent) {
  switch (event.type) {
    case 'approval_required':
    case 'assistant_delta':
    case 'assistant_reasoning_delta':
    case 'assistant_reasoning_done':
    case 'tool_call_output':
    case 'tool_call_started':
      return true
    default:
      return false
  }
}

const keepaliveApiClient = createSlabApiFetchClient({
  fetch: (input, init) =>
    fetch(input, {
      ...init,
      keepalive: true,
    }),
})

type UseAssistantAgentOptions = {
  beforeRequest?: () => Promise<void> | void
  model: string
  reasoningEffort?: AssistantReasoningEffort | null
  reasoningSupported?: boolean
  runtimePresets?: AssistantRuntimePresets | null
  sessionId: string
  systemPrompt?: string | null
  toolConcurrency?: number | null
  toolChoice?: AssistantToolChoice | null
}

export function useAssistantAgent({
  beforeRequest,
  model,
  reasoningEffort,
  reasoningSupported,
  runtimePresets,
  sessionId,
  systemPrompt,
  toolConcurrency,
  toolChoice,
}: UseAssistantAgentOptions) {
  const { t } = useTranslation()
  const locale = useAssistantLocale()
  const resolvedSessionId = sessionId || DEFAULT_CONVERSATION_KEY
  const canLoadSession = !isEphemeralConversationKey(resolvedSessionId)
  const [activeConversation, setActiveConversation] = useState<string>()
  const [messages, setMessages] = useState<AssistantMessageRecord[]>([])
  const [threadId, setThreadId] = useState<string | null>(null)
  const [status, setStatus] = useState<AgentStatus | null>(null)
  const [thoughts, setThoughts] = useState<AssistantThought[]>([])
  const [pendingApprovals, setPendingApprovals] = useState<Map<string, PendingApproval>>(
    () => new Map()
  )
  const [eventsConnected, setEventsConnected] = useState(false)
  const [restoreComplete, setRestoreComplete] = useState(!canLoadSession)
  const socketRef = useRef<WebSocket | null>(null)
  const sseAbortControllerRef = useRef<AbortController | null>(null)
  const sseReconnectTimerRef = useRef<number | null>(null)
  const sseRunIdRef = useRef(0)
  const threadIdRef = useRef<string | null>(null)
  const transportRef = useRef<'none' | 'sse' | 'websocket'>('none')
  const intentionalSocketCloseRef = useRef(false)
  const seenEventIdsRef = useRef<Set<string>>(new Set())
  const lastSeenEventIdRef = useRef<number | null>(null)
  const terminalTurnRef = useRef(false)
  const abortRequestedRef = useRef(false)
  const pendingAbortRef = useRef(false)
  const lastSubmittedPromptRef = useRef<string | null>(null)
  const sessionRef = useRef(resolvedSessionId)
  const handleTransportPayloadRef = useRef<(data: string, eventId?: string | null) => void>(
    () => {}
  )
  const openSseRef = useRef<(threadId: string, attempt?: number) => void>(() => {})
  const postAgentCommandRef = useRef<(command: AgentResponsesClientMessage) => Promise<void>>(
    async () => {}
  )
  const interruptThreadRef = useRef<(threadId: string) => Promise<void>>(async () => {})

  const responsesMutation = api.useMutation('post', '/v1/agents/responses', {
    meta: {
      skipGlobalErrorToast: true,
    },
  })
  const assistantSseResumeEnabled = useGuardrailFlag(GUARDRAIL_PMIDS.assistantSseResume)

  const isRequesting = isBusyStatus(status) || responsesMutation.isPending
  const isHistoryLoading = !restoreComplete

  useEffect(() => {
    threadIdRef.current = threadId
  }, [threadId])

  const clearSseReconnectTimer = useCallback(() => {
    const timer = sseReconnectTimerRef.current
    if (timer) {
      window.clearTimeout(timer)
      sseReconnectTimerRef.current = null
    }
  }, [])

  const closeSse = useCallback(() => {
    clearSseReconnectTimer()
    sseAbortControllerRef.current?.abort()
    sseAbortControllerRef.current = null
    if (transportRef.current === 'sse') {
      transportRef.current = 'none'
    }
  }, [clearSseReconnectTimer])

  const closeSocket = useCallback(() => {
    const socket = socketRef.current
    if (!socket) {
      return
    }

    intentionalSocketCloseRef.current = true
    socket.close()
    socketRef.current = null
    if (transportRef.current === 'websocket') {
      transportRef.current = 'none'
    }
  }, [])

  const closeTransports = useCallback(() => {
    closeSse()
    closeSocket()
    setEventsConnected(false)
  }, [closeSocket, closeSse])

  const markInterrupting = useCallback(() => {
    setStatus('interrupting')
    setThoughts((current) =>
      current.map((thought) =>
        thought.status === 'loading' ? { ...thought, status: 'abort' } : thought
      )
    )
  }, [])

  const clearPendingApproval = useCallback((callId: string) => {
    setPendingApprovals((current) => {
      if (!current.has(callId)) {
        return current
      }

      const next = new Map(current)
      next.delete(callId)
      return next
    })
  }, [])

  const replaceThought = useCallback((nextThought: AssistantThought) => {
    setThoughts((current) => {
      const existingIndex = current.findIndex((thought) => thought.id === nextThought.id)
      if (existingIndex < 0) {
        return [...current.slice(-80), nextThought]
      }

      const next = [...current]
      next[existingIndex] = nextThought
      return next
    })
  }, [])

  const updateThoughtStatus = useCallback(
    (callId: string, statusValue: AssistantThought['status'], detail?: string) => {
      clearPendingApproval(callId)
      setThoughts((current) =>
        current.map((thought) =>
              thought.callId === callId
                ? {
                    ...thought,
                    detail:
                      detail === undefined
                        ? thought.detail
                        : formatKnownToolResult(thought.toolName, detail),
                    pendingApproval: undefined,
                    status: statusValue,
                  }
            : thought
        )
      )
    },
    [clearPendingApproval]
  )

  const appendAssistantDelta = useCallback((text: string) => {
    setMessages((current) => {
      const updated = updateLastAssistantMessage(current, (message) => ({
        ...message,
        message: {
          ...message.message,
          content: `${getAssistantMessageTextContent(message.message)}${text}`,
        },
        status: 'updating',
      }))

      return updated ?? [
        ...current,
        {
          id: nextId('assistant'),
          message: {
            role: 'assistant',
            content: text,
          },
          status: 'updating',
        },
      ]
    })
  }, [])

  const appendAssistantReasoningDelta = useCallback((text: string) => {
    setMessages((current) => {
      const updated = updateLastAssistantMessage(current, (message) => ({
        ...message,
        message: withAssistantMessageReasoningContent(
          message.message,
          `${message.message.reasoningContent ?? ''}${text}`
        ),
        status: 'updating',
      }))

      return updated ?? [
        ...current,
        {
          id: nextId('assistant'),
          message: withAssistantMessageReasoningContent(
            {
              role: 'assistant',
              content: '',
            },
            text
          ),
          status: 'updating',
        },
      ]
    })
  }, [])

  const completeAssistantReasoning = useCallback((text: string) => {
    setMessages((current) => {
      const updated = updateLastAssistantMessage(current, (message) => ({
        ...message,
        message: withAssistantMessageReasoningContent(message.message, text),
        status: message.status === 'loading' ? 'updating' : message.status,
      }))

      if (updated) {
        return updated
      }

      if (!text.trim()) {
        return current
      }

      return [
        ...current,
        {
          id: nextId('assistant'),
          message: withAssistantMessageReasoningContent(
            {
              role: 'assistant',
              content: '',
            },
            text
          ),
          status: 'updating',
        },
      ]
    })
  }, [])

  const completeAssistantTurn = useCallback((text: string, artifactRefs: AssistantArtifactRef[] = []) => {
    setMessages((current) => {
      const cleanedText = stripTrailingAssistantTurnArtifacts(text)
      const updated = updateLastAssistantMessage(current, (message) => ({
        ...message,
        message: {
          ...message.message,
          artifactRefs: artifactRefs.length > 0 ? artifactRefs : message.message.artifactRefs,
          content: cleanedText || getAssistantMessageTextContent(message.message),
        },
        status: 'success',
      }))

      if (updated) {
        return updated
      }

      if (!cleanedText.trim()) {
        return current
      }

      return [
        ...current,
        {
          id: nextId('assistant'),
          message: {
            artifactRefs: artifactRefs.length > 0 ? artifactRefs : undefined,
            role: 'assistant',
            content: cleanedText,
          },
          status: 'success',
        },
      ]
    })
  }, [])

  const appendAssistantError = useCallback((content: string) => {
    setMessages((current) => {
      const updated = updateLastAssistantMessage(current, (message) => ({
        ...message,
        message: {
          ...message.message,
          content,
        },
        status: 'error',
      }))

      return updated ?? [
        ...current,
        {
          id: nextId('assistant'),
          message: {
            role: 'assistant',
            content,
          },
          status: 'error',
        },
      ]
    })
  }, [])

  const markAssistantTurnFailed = useCallback((message: string) => {
    setMessages((current) => {
      const updated = updateLastAssistantMessage(current, (record) => ({
        ...record,
        message: {
          ...record.message,
          terminalNotice: {
            message,
            type: 'error',
          },
        },
        status: 'error',
      }))

      return updated ?? [
        ...current,
        {
          id: nextId('assistant'),
          message: {
            role: 'assistant',
            content: '',
            terminalNotice: {
              message,
              type: 'error',
            },
          },
          status: 'error',
        },
      ]
    })
  }, [])

  const markAssistantTurnCancelled = useCallback((message: string) => {
    setMessages((current) => {
      const updated = updateLastAssistantMessage(current, (record) => ({
        ...record,
        message: {
          ...record.message,
          terminalNotice: {
            message,
            type: 'cancelled',
          },
        },
        status: 'abort',
      }))

      return updated ?? [
        ...current,
        {
          id: nextId('assistant'),
          message: {
            role: 'assistant',
            content: '',
            terminalNotice: {
              message,
              type: 'cancelled',
            },
          },
          status: 'abort',
        },
      ]
    })
  }, [])

  const handleAgentEvent = useCallback(
    (data: string) => {
      const event = parseAssistantAgentStreamEvent(data)
      if (!event) {
        return
      }
      if (abortRequestedRef.current && shouldIgnoreAfterAbort(event)) {
        return
      }

      switch (event.type) {
        case 'agent_status':
          setStatus(event.status)
          break
        case 'approval_required':
          setPendingApprovals((current) => {
            const next = new Map(current)
            next.set(event.call_id, {
              callId: event.call_id,
              toolName: event.tool_name,
              command: event.command,
            })
            return next
          })
          replaceThought({
            id: event.call_id,
            callId: event.call_id,
            detail: event.command,
            pendingApproval: {
              callId: event.call_id,
              command: event.command,
              toolName: event.tool_name,
            },
            status: 'loading',
            summary: `tool_call id=${event.call_id}: ${event.tool_name}(${event.command})`,
            title: `${event.tool_name} approval`,
            toolName: event.tool_name,
          })
          break
        case 'assistant_delta':
          appendAssistantDelta(event.text)
          break
        case 'assistant_reasoning_delta':
          appendAssistantReasoningDelta(event.text)
          break
        case 'assistant_reasoning_done':
          completeAssistantReasoning(event.text)
          break
        case 'turn_cancelled':
          terminalTurnRef.current = true
          pendingAbortRef.current = false
          abortRequestedRef.current = false
          setStatus('interrupted')
          setPendingApprovals(new Map())
          markAssistantTurnCancelled(event.reason)
          setThoughts((current) =>
            current.map((thought) =>
              thought.status === 'loading' ? { ...thought, status: 'abort' } : thought
            )
          )
          break
        case 'lagged':
          replaceThought({
            id: nextId('lagged'),
            status: 'error',
            title: locale.eventStreamLagged,
          })
          break
        case 'tool_call_output':
          updateThoughtStatus(event.call_id, 'success', event.output)
          break
        case 'tool_call_started': {
          const dispatch = dispatchA2uToolCall(event.tool_name, event.arguments)
          if (dispatch) {
            useAgentSurfaceStore.getState().setPendingSurface(dispatch.surface)
            break
          }

          replaceThought({
            id: event.call_id,
            callId: event.call_id,
            detail: event.arguments,
            status: 'loading',
            summary: `tool_call id=${event.call_id}: ${event.tool_name}(${event.arguments})`,
            title: 'tool_call',
            toolName: event.tool_name,
          })
          break
        }
        case 'turn_completed':
        case 'turn_finished':
          terminalTurnRef.current = true
          pendingAbortRef.current = false
          abortRequestedRef.current = false
          setStatus('completed')
          setPendingApprovals(new Map())
          setThoughts((current) =>
            current.map((thought) =>
              thought.status === 'loading' ? { ...thought, status: 'success' } : thought
            )
          )
          if (event.type === 'turn_completed') {
            completeAssistantTurn(event.text, event.artifact_refs)
          }
          break
        case 'turn_failed':
          terminalTurnRef.current = true
          pendingAbortRef.current = false
          abortRequestedRef.current = false
          setStatus('errored')
          setPendingApprovals(new Map())
          setThoughts((current) =>
            current.map((thought) =>
              thought.status === 'loading' ? { ...thought, status: 'error' } : thought
            )
          )
          markAssistantTurnFailed(event.error)
          break
      }
    },
    [
      appendAssistantDelta,
      appendAssistantReasoningDelta,
      completeAssistantReasoning,
      completeAssistantTurn,
      locale.eventStreamLagged,
      markAssistantTurnCancelled,
      markAssistantTurnFailed,
      replaceThought,
      updateThoughtStatus,
    ]
  )

  const handleServerMessage = useCallback(
    (message: AgentResponsesServerMessage) => {
      switch (message.type) {
        case 'agent.ack':
          if (message.thread_id) {
            setThreadId(message.thread_id)
            if (pendingAbortRef.current) {
              pendingAbortRef.current = false
              closeTransports()
              void interruptThreadRef.current(message.thread_id)
            }
          }
          if (message.status) {
            setStatus(message.status)
          }
          if (message.action === 'approval_resolve' && message.delivered === false) {
            toast.error(locale.approvalNotDelivered)
          }
          break
        case 'agent.session.restored':
          setRestoreComplete(true)
          setActiveConversation(message.session_id)
          setThreadId(message.thread?.id ?? null)
          setMessages(
            projectAgentThreadMessages(message.messages, message.thread?.status ?? undefined)
          )
          setStatus(message.thread?.status ?? null)
          break
        case 'agent.error': {
          const errorMessage = translateServerField(message.i18n, 'message', message.message, t)
          setRestoreComplete(true)
          setStatus('errored')
          appendAssistantError(errorMessage)
          toast.error(locale.requestFailed, {
            description: errorMessage,
          })
          break
        }
      }
    },
    [appendAssistantError, closeTransports, locale.approvalNotDelivered, locale.requestFailed, t]
  )

  const handleTransportPayload = useCallback(
    (data: string, eventId?: string | null) => {
      if (eventId) {
        const sequenceNumber = Number(eventId)
        if (Number.isFinite(sequenceNumber)) {
          lastSeenEventIdRef.current = Math.max(
            lastSeenEventIdRef.current ?? 0,
            sequenceNumber
          )
        }
      }

      const serverMessage = parseAssistantAgentServerMessage(data)
      if (serverMessage) {
        handleServerMessage(serverMessage)
        return
      }

      const key = agentEventKey(data)
      if (key) {
        if (seenEventIdsRef.current.has(key)) {
          return
        }
        seenEventIdsRef.current.add(key)
        const sequenceNumber = Number(key.split(':').at(-1))
        if (Number.isFinite(sequenceNumber)) {
          lastSeenEventIdRef.current = Math.max(
            lastSeenEventIdRef.current ?? 0,
            sequenceNumber
          )
        }
      }
      handleAgentEvent(data)
    },
    [handleAgentEvent, handleServerMessage]
  )

  const openSse = useCallback(
    (nextThreadId: string, attempt = 0) => {
      closeSse()
      const controller = new AbortController()
      const runId = sseRunIdRef.current + 1
      sseRunIdRef.current = runId
      sseAbortControllerRef.current = controller
      transportRef.current = 'sse'
      void readAssistantSseStream(agentResponsesSseUrl(nextThreadId), {
        lastEventId: assistantSseResumeEnabled ? lastSeenEventIdRef.current : null,
        onMessage: (message) => {
          handleTransportPayload(message.data, message.id)
        },
        onOpen: () => {
          if (sseRunIdRef.current === runId) {
            setEventsConnected(true)
          }
        },
        signal: controller.signal,
      }).catch((error) => {
        if (
          sseRunIdRef.current !== runId ||
          controller.signal.aborted ||
          isAbortError(error) ||
          terminalTurnRef.current
        ) {
          return
        }

        setEventsConnected(false)
        if (attempt >= MAX_SSE_RECONNECT_ATTEMPTS) {
          toast.error(locale.eventStreamInterrupted, {
            id: `assistant-sse-disconnected:${nextThreadId}`,
          })
          return
        }

        sseReconnectTimerRef.current = window.setTimeout(() => {
          openSseRef.current(nextThreadId, attempt + 1)
        }, nextReconnectDelayMs(attempt))
      })
    },
    [assistantSseResumeEnabled, closeSse, handleTransportPayload, locale.eventStreamInterrupted]
  )

  const postAgentCommand = useCallback(
    async (command: AgentResponsesClientMessage) => {
      const response = await responsesMutation.mutateAsync({
        body: command,
      })
      handleServerMessage(response)
      const nextThreadId = serverMessageThreadId(response)
      if (nextThreadId) {
        openSse(nextThreadId)
      }
    },
    [handleServerMessage, openSse, responsesMutation]
  )

  useEffect(() => {
    handleTransportPayloadRef.current = handleTransportPayload
  }, [handleTransportPayload])

  useEffect(() => {
    openSseRef.current = openSse
  }, [openSse])

  useEffect(() => {
    postAgentCommandRef.current = postAgentCommand
  }, [postAgentCommand])

  const sendAgentCommand = useCallback(async (command: AgentResponsesClientMessage) => {
    const socket = socketRef.current
    if (socket?.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify(command))
      return
    }

    await postAgentCommandRef.current(command)
  }, [])

  useEffect(() => {
    sessionRef.current = resolvedSessionId
    closeTransports()
    sseRunIdRef.current += 1
    transportRef.current = 'none'
    seenEventIdsRef.current.clear()
    lastSeenEventIdRef.current = null
    terminalTurnRef.current = false
    abortRequestedRef.current = false
    pendingAbortRef.current = false
    setActiveConversation(undefined)
    setMessages([])
    setThreadId(null)
    setStatus(null)
    setThoughts([])
    setPendingApprovals(new Map())
    setEventsConnected(false)
    setRestoreComplete(!canLoadSession)

    if (!canLoadSession) {
      return undefined
    }

    const socket = new WebSocket(agentResponsesWebSocketUrl())
    socketRef.current = socket
    transportRef.current = 'websocket'
    let opened = false
    let fallbackStarted = false
    let disposed = false

    const fallbackRestore = () => {
      if (fallbackStarted || disposed) {
        return
      }
      fallbackStarted = true
      transportRef.current = 'none'
      void postAgentCommandRef.current({
        request_id: nextId('request'),
        session_id: resolvedSessionId,
        type: 'agent.session.restore',
      })
    }

    socket.addEventListener('open', () => {
      opened = true
      setEventsConnected(true)
      socket.send(
        JSON.stringify({
          request_id: nextId('request'),
          session_id: resolvedSessionId,
          type: 'agent.session.restore',
        } satisfies AgentResponsesClientMessage)
      )
    })
    socket.addEventListener('message', (event) => {
      handleTransportPayloadRef.current(String(event.data))
    })
    socket.addEventListener('error', () => {
      setEventsConnected(false)
      if (!opened) {
        fallbackRestore()
      }
    })
    socket.addEventListener('close', () => {
      if (socketRef.current === socket) {
        socketRef.current = null
      }
      if (transportRef.current === 'websocket') {
        transportRef.current = 'none'
      }
      setEventsConnected(false)
      if (intentionalSocketCloseRef.current) {
        intentionalSocketCloseRef.current = false
        return
      }
      if (disposed) {
        return
      }
      if (!opened) {
        fallbackRestore()
        return
      }

      const activeThreadId = threadIdRef.current
      if (activeThreadId) {
        openSseRef.current(activeThreadId)
      }
    })

    return () => {
      disposed = true
      const activeThreadId = threadIdRef.current
      intentionalSocketCloseRef.current = true
      socket.close()
      closeSse()
      if (socketRef.current === socket) {
        socketRef.current = null
      }
      transportRef.current = 'none'
      setEventsConnected(false)
      if (activeThreadId) {
        void keepaliveApiClient.POST('/v1/agents/responses', {
          body: {
            request_id: nextId('request'),
            thread_id: activeThreadId,
            type: 'agent.shutdown',
          },
        }).catch(() => {})
      }
    }
  }, [canLoadSession, closeSse, closeTransports, resolvedSessionId])

  const handleSubmit = useCallback(
    async (value: string) => {
      const prompt = value.trim()
      if (!prompt || isRequesting || !canLoadSession) {
        return
      }
      const slashCommand = parseAssistantSlashCommand(prompt)
      const submittedPrompt = slashCommand?.content || prompt
      lastSubmittedPromptRef.current = submittedPrompt
      terminalTurnRef.current = false
      abortRequestedRef.current = false
      pendingAbortRef.current = false

      const userMessage: AssistantMessageRecord = {
        id: nextId('user'),
        message: {
          role: 'user',
          content: submittedPrompt,
        },
        status: 'success',
      }

      setMessages((current) => [...current, userMessage])
      setStatus('pending')
      setPendingApprovals(new Map())
      setThoughts([])
      setActiveConversation(resolvedSessionId)

      try {
        await beforeRequest?.()
      } catch (error) {
        const message = getErrorMessage(error)
        setStatus('errored')
        appendAssistantError(message || locale.requestFailed)
        return
      }

      setMessages((current) => [
        ...current,
        {
          id: nextId('assistant'),
          message: {
            role: 'assistant',
            content: '',
          },
          status: 'loading',
        },
      ])

      try {
        if (!threadId) {
          const requestMessages = toAssistantRequestMessages([
            ...messages.map((message) => message.message),
            userMessage.message,
          ])
          await sendAgentCommand({
            config: toAgentConfig({
              model,
              reasoningEffort,
              reasoningSupported,
              runtimePresets,
              slashCommand,
              systemPrompt,
              toolChoice,
              toolConcurrency,
            }),
            messages: requestMessages,
            request_id: nextId('request'),
            session_id: resolvedSessionId,
            type: 'agent.response.create',
          })
          return
        }

        await sendAgentCommand({
          content: submittedPrompt,
          request_id: nextId('request'),
          thread_id: threadId,
          type: 'agent.input',
        })
        setStatus('running')
      } catch (error) {
        const message = getErrorMessage(error)
        setStatus('errored')
        appendAssistantError(message || locale.requestFailed)
        toast.error(locale.requestFailed, {
          description: message,
        })
      }
    },
    [
      appendAssistantError,
      beforeRequest,
      canLoadSession,
      isRequesting,
      locale.requestFailed,
      messages,
      model,
      reasoningEffort,
      reasoningSupported,
      resolvedSessionId,
      runtimePresets,
      sendAgentCommand,
      systemPrompt,
      threadId,
      toolChoice,
      toolConcurrency,
    ]
  )

  const startBranchedResponse = useCallback(
    async (nextMessages: AssistantMessageRecord[], prompt: string) => {
      const submittedPrompt = prompt.trim()
      if (!submittedPrompt || isRequesting || !canLoadSession) {
        return
      }

      const slashCommand = parseAssistantSlashCommand(submittedPrompt)
      const requestMessages = toAssistantRequestMessages(nextMessages.map((message) => message.message))
      if (requestMessages.length === 0) {
        return
      }

      lastSubmittedPromptRef.current = slashCommand?.content || submittedPrompt
      terminalTurnRef.current = false
      abortRequestedRef.current = false
      pendingAbortRef.current = false
      closeTransports()
      setThreadId(null)
      setMessages(nextMessages)
      setStatus('pending')
      setPendingApprovals(new Map())
      setThoughts([])
      setActiveConversation(resolvedSessionId)

      try {
        await beforeRequest?.()
      } catch (error) {
        const message = getErrorMessage(error)
        setStatus('errored')
        appendAssistantError(message || locale.requestFailed)
        return
      }

      setMessages([
        ...nextMessages,
        {
          id: nextId('assistant'),
          message: {
            role: 'assistant',
            content: '',
          },
          status: 'loading',
        },
      ])

      try {
        await sendAgentCommand({
          config: toAgentConfig({
            model,
            reasoningEffort,
            reasoningSupported,
            runtimePresets,
            slashCommand,
            systemPrompt,
            toolChoice,
            toolConcurrency,
          }),
          messages: requestMessages,
          request_id: nextId('request'),
          session_id: resolvedSessionId,
          type: 'agent.response.create',
        })
      } catch (error) {
        const message = getErrorMessage(error)
        setStatus('errored')
        appendAssistantError(message || locale.requestFailed)
        toast.error(locale.requestFailed, {
          description: message,
        })
      }
    },
    [
      appendAssistantError,
      beforeRequest,
      canLoadSession,
      closeTransports,
      isRequesting,
      locale.requestFailed,
      model,
      reasoningEffort,
      reasoningSupported,
      resolvedSessionId,
      runtimePresets,
      sendAgentCommand,
      systemPrompt,
      toolChoice,
      toolConcurrency,
    ]
  )

  const editAndResend = useCallback(
    async (messageId: string, nextContent: string) => {
      const trimmed = nextContent.trim()
      if (!trimmed) {
        return
      }

      const targetIndex = messages.findIndex((message) => message.id === messageId)
      const target = messages[targetIndex]
      if (!target || target.message.role !== 'user') {
        return
      }

      const nextMessages = messages.slice(0, targetIndex + 1)
      nextMessages[targetIndex] = {
        ...target,
        message: {
          ...target.message,
          content: trimmed,
        },
        status: 'success',
      }
      await startBranchedResponse(nextMessages, trimmed)
    },
    [messages, startBranchedResponse]
  )

  const regenerateResponse = useCallback(
    async (messageId?: string) => {
      const assistantIndex =
        typeof messageId === 'string'
          ? messages.findIndex((message) => message.id === messageId)
          : messages.findLastIndex((message) => message.message.role === 'assistant')
      const assistantMessage = messages[assistantIndex]
      if (!assistantMessage || assistantMessage.message.role !== 'assistant') {
        return
      }

      const userIndex = messages
        .slice(0, assistantIndex)
        .findLastIndex((message) => message.message.role === 'user')
      const userMessage = messages[userIndex]
      if (!userMessage) {
        return
      }

      const prompt = getAssistantMessageTextContent(userMessage.message)
      await startBranchedResponse(messages.slice(0, assistantIndex), prompt)
    },
    [messages, startBranchedResponse]
  )

  const submitApproval = useCallback(
    async (callId: string, approved: boolean) => {
      const decision = pendingApprovals.get(callId)
      if (!threadId || !decision) {
        return
      }

      clearPendingApproval(callId)
      updateThoughtStatus(decision.callId, approved ? 'loading' : 'abort')

      try {
        await sendAgentCommand({
          approved,
          call_id: decision.callId,
          request_id: nextId('request'),
          thread_id: threadId,
          type: 'agent.approval.resolve',
        })
      } catch (error) {
        toast.error(locale.approvalFailed, {
          description: getErrorMessage(error),
        })
      }
    },
    [
      clearPendingApproval,
      locale.approvalFailed,
      pendingApprovals,
      sendAgentCommand,
      threadId,
      updateThoughtStatus,
    ]
  )

  const interruptThread = useCallback(
    async (activeThreadId: string) => {
      markInterrupting()
      try {
        await postAgentCommandRef.current({
          request_id: nextId('request'),
          thread_id: activeThreadId,
          type: 'agent.interrupt',
        })
      } catch (error) {
        toast.error(locale.interruptFailed, {
          description: getErrorMessage(error),
        })
      }
    },
    [locale.interruptFailed, markInterrupting]
  )

  useEffect(() => {
    interruptThreadRef.current = interruptThread
  }, [interruptThread])

  const abort = useCallback(() => {
    if (!isRequesting) {
      return
    }

    abortRequestedRef.current = true
    terminalTurnRef.current = false
    markInterrupting()

    const activeThreadId = threadIdRef.current
    if (!activeThreadId) {
      pendingAbortRef.current = true
      closeSse()
      return
    }

    closeTransports()
    void interruptThread(activeThreadId)
  }, [closeSse, closeTransports, interruptThread, isRequesting, markInterrupting])

  const retryLastResponse = useCallback(() => {
    const prompt = lastSubmittedPromptRef.current
    if (!prompt || isRequesting) {
      return
    }

    void handleSubmit(prompt)
  }, [handleSubmit, isRequesting])

  const resume = useCallback(() => {
    if (isRequesting) {
      return
    }
    // TC-FE-05: resume the active (interrupted) thread with a fresh continue
    // cue instead of re-sending the original prompt.
    void handleSubmit('Continue from where you left off.')
  }, [handleSubmit, isRequesting])

  const messagesWithThoughts = useMemo(
    () => withThoughts(messages, thoughts),
    [messages, thoughts]
  )
  const pendingApprovalList = useMemo(
    () => Array.from(pendingApprovals.values()),
    [pendingApprovals]
  )
  // TC-FE-05: latest plan_update progress (X/N) for the progress bar.
  const planProgress = useMemo<PlanProgress | null>(() => {
    for (let i = thoughts.length - 1; i >= 0; i -= 1) {
      const thought = thoughts[i]
      if (thought.toolName === 'plan_update' && thought.detail) {
        const progress = parsePlanProgress(thought.detail)
        if (progress) {
          return progress
        }
      }
    }
    return null
  }, [thoughts])
  // TC-FE-05: structured termination reason from the last cancelled turn, for
  // the resume affordance.
  const terminalReason = useMemo<string | null>(() => {
    for (let i = messages.length - 1; i >= 0; i -= 1) {
      const notice = messages[i]?.message?.terminalNotice
      if (notice?.type === 'cancelled' && notice.message) {
        return notice.message
      }
    }
    return null
  }, [messages])

  return {
    abort,
    activeConversation,
    editAndResend,
    eventsConnected,
    handleSubmit,
    isHistoryLoading,
    isRequesting,
    messages: messagesWithThoughts,
    pendingApprovals: pendingApprovalList,
    planProgress,
    regenerateResponse,
    resume,
    retryLastResponse,
    status,
    terminalReason,
    submitApproval,
    threadId,
  }
}
