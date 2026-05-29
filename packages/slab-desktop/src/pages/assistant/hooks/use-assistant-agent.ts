import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'

import api, { getErrorMessage } from '@slab/api'
import { SERVER_BASE_URL } from '@slab/api/config'

import {
  DEFAULT_CONVERSATION_KEY,
  getAssistantMessageTextContent,
  isEphemeralConversationKey,
  stripTrailingAssistantTurnArtifacts,
  toAssistantRequestMessages,
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
} from '../lib/assistant-agent-events'
import { withAssistantMessageReasoningContent } from '../lib/assistant-message-utils'
import {
  projectAgentThreadMessages,
  projectSessionMessages,
} from '../lib/assistant-message-projection'

type PendingApproval = {
  callId: string
  toolName: string
  command: string
}

type UseAssistantAgentOptions = {
  beforeRequest?: () => Promise<void> | void
  deepThink?: boolean
  model: string
  runtimePresets?: AssistantRuntimePresets | null
  sessionId: string
}

function nextId(prefix: string) {
  const random =
    typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
      ? crypto.randomUUID()
      : `${Date.now()}-${Math.random().toString(36).slice(2)}`

  return `${prefix}-${random}`
}

function isBusyStatus(status: AgentStatus | null) {
  return status === 'pending' || status === 'running' || status === 'interrupting'
}

function toAgentConfig(
  model: string,
  runtimePresets?: AssistantRuntimePresets | null,
  deepThink?: boolean
) {
  return {
    allowed_tools: [] as string[],
    max_turns: 8,
    model,
    ...(typeof runtimePresets?.max_tokens === 'number'
      ? { max_tokens: runtimePresets.max_tokens }
      : {}),
    ...(typeof runtimePresets?.temperature === 'number'
      ? { temperature: runtimePresets.temperature }
      : {}),
    ...(typeof runtimePresets?.top_p === 'number' ? { top_p: runtimePresets.top_p } : {}),
    ...(typeof runtimePresets?.top_k === 'number' ? { top_k: runtimePresets.top_k } : {}),
    ...(typeof runtimePresets?.min_p === 'number' ? { min_p: runtimePresets.min_p } : {}),
    ...(typeof runtimePresets?.presence_penalty === 'number'
      ? { presence_penalty: runtimePresets.presence_penalty }
      : {}),
    ...(typeof runtimePresets?.repetition_penalty === 'number'
      ? { repetition_penalty: runtimePresets.repetition_penalty }
      : {}),
    ...(deepThink ? { reasoning_effort: 'medium' as const } : {}),
  }
}

function updateLastAssistantMessage(
  messages: AssistantMessageRecord[],
  updater: (message: AssistantMessageRecord) => AssistantMessageRecord
) {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index]
    if (message?.message.role === 'assistant' && message.status !== 'success') {
      const next = [...messages]
      next[index] = updater(message)
      return next
    }
  }

  return null
}

function withThoughts(
  messages: AssistantMessageRecord[],
  thoughts: AssistantThought[]
): AssistantMessageRecord[] {
  if (thoughts.length === 0) {
    return messages
  }

  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index]
    if (message?.message.role !== 'assistant') {
      continue
    }

    const next = [...messages]
    next[index] = {
      ...message,
      message: {
        ...message.message,
        thoughts,
      },
    }
    return next
  }

  return [
    ...messages,
    {
      id: nextId('assistant'),
      message: {
        role: 'assistant',
        content: '',
        thoughts,
      },
      status: 'loading',
    },
  ]
}

function agentResponsesWebSocketUrl() {
  const endpoint = new URL(SERVER_BASE_URL)
  endpoint.protocol = endpoint.protocol === 'https:' ? 'wss:' : 'ws:'
  endpoint.pathname = '/v1/agents/responses'
  endpoint.search = ''
  endpoint.hash = ''
  return endpoint.toString()
}

function agentResponsesSseUrl(threadId: string) {
  const endpoint = new URL(SERVER_BASE_URL)
  endpoint.pathname = '/v1/agents/responses'
  endpoint.search = ''
  endpoint.searchParams.set('transport', 'sse')
  endpoint.searchParams.set('thread_id', threadId)
  endpoint.hash = ''
  return endpoint.toString()
}

function agentEventKey(data: string): string | null {
  try {
    const value = JSON.parse(data) as unknown
    if (
      typeof value === 'object' &&
      value !== null &&
      'thread_id' in value &&
      'sequence_number' in value
    ) {
      const threadId = (value as { thread_id?: unknown }).thread_id
      const sequenceNumber = (value as { sequence_number?: unknown }).sequence_number
      if (typeof threadId === 'string' && typeof sequenceNumber === 'number') {
        return `${threadId}:${sequenceNumber}`
      }
    }
  } catch {
    return null
  }

  return null
}

function serverMessageThreadId(message: AgentResponsesServerMessage): string | null {
  switch (message.type) {
    case 'agent.ack':
      return message.thread_id ?? null
    case 'agent.session.restored':
      return message.thread?.id ?? null
    case 'agent.error':
      return message.thread_id ?? null
    default:
      return null
  }
}

export function useAssistantAgent({
  beforeRequest,
  deepThink,
  model,
  runtimePresets,
  sessionId,
}: UseAssistantAgentOptions) {
  const locale = useAssistantLocale()
  const resolvedSessionId = sessionId || DEFAULT_CONVERSATION_KEY
  const canLoadSession = !isEphemeralConversationKey(resolvedSessionId)
  const [activeConversation, setActiveConversation] = useState<string>()
  const [messages, setMessages] = useState<AssistantMessageRecord[]>([])
  const [threadId, setThreadId] = useState<string | null>(null)
  const [status, setStatus] = useState<AgentStatus | null>(null)
  const [thoughts, setThoughts] = useState<AssistantThought[]>([])
  const [pendingApproval, setPendingApproval] = useState<PendingApproval | null>(null)
  const [eventsConnected, setEventsConnected] = useState(false)
  const [restoreComplete, setRestoreComplete] = useState(!canLoadSession)
  const eventSourceRef = useRef<EventSource | null>(null)
  const socketRef = useRef<WebSocket | null>(null)
  const threadIdRef = useRef<string | null>(null)
  const transportRef = useRef<'none' | 'sse' | 'websocket'>('none')
  const seenEventIdsRef = useRef<Set<string>>(new Set())
  const sessionRef = useRef(resolvedSessionId)
  const handleTransportPayloadRef = useRef<(data: string) => void>(() => {})
  const openSseRef = useRef<(threadId: string) => void>(() => {})
  const postAgentCommandRef = useRef<(command: AgentResponsesClientMessage) => Promise<void>>(
    async () => {}
  )

  const {
    data: sessionMessages,
    isLoading: isSessionMessagesLoading,
  } = api.useQuery(
    'get',
    '/v1/sessions/{id}/messages',
    {
      params: {
        path: {
          id: resolvedSessionId,
        },
      },
    },
    {
      enabled: canLoadSession && restoreComplete && !threadId,
      retry: false,
    }
  )

  const responsesMutation = api.useMutation('post', '/v1/agents/responses')

  const isRequesting = isBusyStatus(status) || responsesMutation.isPending
  const isHistoryLoading = !restoreComplete || (!threadId && isSessionMessagesLoading)

  useEffect(() => {
    threadIdRef.current = threadId
  }, [threadId])

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
      setThoughts((current) =>
        current.map((thought) =>
          thought.callId === callId
            ? {
                ...thought,
                detail: detail ?? thought.detail,
                pendingApproval: undefined,
                status: statusValue,
              }
            : thought
        )
      )
    },
    []
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

  const completeAssistantTurn = useCallback((text: string) => {
    setMessages((current) => {
      const cleanedText = stripTrailingAssistantTurnArtifacts(text)
      const updated = updateLastAssistantMessage(current, (message) => ({
        ...message,
        message: {
          ...message.message,
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

  const handleAgentEvent = useCallback(
    (data: string) => {
      const event = parseAssistantAgentStreamEvent(data)
      if (!event) {
        return
      }

      switch (event.type) {
        case 'agent_status':
          setStatus(event.status)
          break
        case 'approval_required':
          setPendingApproval({
            callId: event.call_id,
            toolName: event.tool_name,
            command: event.command,
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
          setStatus('interrupted')
          setPendingApproval(null)
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
        case 'tool_call_started':
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
        case 'turn_completed':
        case 'turn_finished':
          setStatus('completed')
          setPendingApproval(null)
          setThoughts((current) =>
            current.map((thought) =>
              thought.status === 'loading' ? { ...thought, status: 'success' } : thought
            )
          )
          if (event.type === 'turn_completed') {
            completeAssistantTurn(event.text)
          }
          break
        case 'turn_failed':
          setStatus('errored')
          setPendingApproval(null)
          setThoughts((current) =>
            current.map((thought) =>
              thought.status === 'loading' ? { ...thought, status: 'error' } : thought
            )
          )
          appendAssistantError(event.error)
          break
      }
    },
    [
      appendAssistantDelta,
      appendAssistantError,
      appendAssistantReasoningDelta,
      completeAssistantReasoning,
      completeAssistantTurn,
      locale.eventStreamLagged,
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
          setMessages(projectAgentThreadMessages(message.messages))
          setStatus(message.thread?.status ?? null)
          break
        case 'agent.error':
          setRestoreComplete(true)
          setStatus('errored')
          appendAssistantError(message.message)
          toast.error(locale.requestFailed, {
            description: message.message,
          })
          break
      }
    },
    [appendAssistantError, locale.approvalNotDelivered, locale.requestFailed]
  )

  const handleTransportPayload = useCallback(
    (data: string) => {
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
      }
      handleAgentEvent(data)
    },
    [handleAgentEvent, handleServerMessage]
  )

  const openSse = useCallback(
    (nextThreadId: string) => {
      eventSourceRef.current?.close()
      const source = new EventSource(agentResponsesSseUrl(nextThreadId))
      eventSourceRef.current = source
      transportRef.current = 'sse'
      source.addEventListener('open', () => setEventsConnected(true))
      source.addEventListener('error', () => setEventsConnected(false))
      source.addEventListener('message', (message: MessageEvent<string>) => {
        handleTransportPayload(message.data)
      })
    },
    [handleTransportPayload]
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
    socketRef.current?.close()
    eventSourceRef.current?.close()
    socketRef.current = null
    eventSourceRef.current = null
    transportRef.current = 'none'
    seenEventIdsRef.current.clear()
    setActiveConversation(undefined)
    setMessages([])
    setThreadId(null)
    setStatus(null)
    setThoughts([])
    setPendingApproval(null)
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
      socket.close()
      eventSourceRef.current?.close()
      if (socketRef.current === socket) {
        socketRef.current = null
      }
      eventSourceRef.current = null
      transportRef.current = 'none'
      setEventsConnected(false)
    }
  }, [canLoadSession, resolvedSessionId])

  useEffect(() => {
    if (isRequesting || isHistoryLoading || threadId) {
      return
    }

    setMessages(projectSessionMessages(sessionMessages))
    setStatus(null)
  }, [isHistoryLoading, isRequesting, sessionMessages, threadId])

  const handleSubmit = useCallback(
    async (value: string) => {
      const prompt = value.trim()
      if (!prompt || isRequesting || !canLoadSession) {
        return
      }

      try {
        await beforeRequest?.()
      } catch {
        return
      }

      const userMessage: AssistantMessageRecord = {
        id: nextId('user'),
        message: {
          role: 'user',
          content: prompt,
        },
        status: 'success',
      }

      setMessages((current) => [
        ...current,
        userMessage,
        {
          id: nextId('assistant'),
          message: {
            role: 'assistant',
            content: '',
          },
          status: 'loading',
        },
      ])
      setStatus('pending')
      setPendingApproval(null)
      setThoughts([])
      setActiveConversation(resolvedSessionId)

      try {
        if (!threadId) {
          const requestMessages = toAssistantRequestMessages([
            ...messages.map((message) => message.message),
            userMessage.message,
          ])
          await sendAgentCommand({
            config: toAgentConfig(model, runtimePresets, deepThink),
            messages: requestMessages,
            request_id: nextId('request'),
            session_id: resolvedSessionId,
            type: 'agent.response.create',
          })
          return
        }

        await sendAgentCommand({
          content: prompt,
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
      deepThink,
      isRequesting,
      locale.requestFailed,
      messages,
      model,
      resolvedSessionId,
      runtimePresets,
      sendAgentCommand,
      threadId,
    ]
  )

  const submitApproval = useCallback(
    async (approved: boolean) => {
      if (!threadId || !pendingApproval) {
        return
      }

      const decision = pendingApproval
      setPendingApproval(null)
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
      locale.approvalFailed,
      pendingApproval,
      sendAgentCommand,
      threadId,
      updateThoughtStatus,
    ]
  )

  const abort = useCallback(() => {
    if (!threadId || !isRequesting) {
      return
    }

    void sendAgentCommand({
      request_id: nextId('request'),
      thread_id: threadId,
      type: 'agent.interrupt',
    })
      .then(() => {
        setStatus('interrupting')
        setThoughts((current) =>
          current.map((thought) =>
            thought.status === 'loading' ? { ...thought, status: 'abort' } : thought
          )
        )
      })
      .catch((error) => {
        toast.error(locale.interruptFailed, {
          description: getErrorMessage(error),
        })
      })
  }, [isRequesting, locale.interruptFailed, sendAgentCommand, threadId])

  const messagesWithThoughts = useMemo(
    () => withThoughts(messages, thoughts),
    [messages, thoughts]
  )

  return {
    abort,
    activeConversation,
    eventsConnected,
    handleSubmit,
    isHistoryLoading,
    isRequesting,
    messages: messagesWithThoughts,
    pendingApproval,
    status,
    submitApproval,
    threadId,
  }
}
