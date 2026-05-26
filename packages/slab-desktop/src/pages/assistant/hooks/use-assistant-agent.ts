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
  type AgentStatus,
  type AssistantMessageRecord,
  type AssistantRuntimePresets,
  type AssistantThought,
} from '../assistant-context'
import { useAssistantLocale } from '../assistant-locale'
import { parseAssistantAgentStreamEvent } from '../lib/assistant-agent-events'
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
  return status === 'pending' || status === 'running'
}

function toAgentConfig(model: string, runtimePresets?: AssistantRuntimePresets | null) {
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

export function useAssistantAgent({
  beforeRequest,
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
  const eventSourceRef = useRef<EventSource | null>(null)
  const seenEventIdsRef = useRef<Set<string>>(new Set())
  const sessionRef = useRef(resolvedSessionId)

  const {
    data: sessionThreads,
    isLoading: isThreadsLoading,
    refetch: refetchSessionThreads,
  } = api.useQuery(
    'get',
    '/v1/agents/session/{session_id}/threads',
    {
      params: {
        path: {
          session_id: resolvedSessionId,
        },
      },
    },
    {
      enabled: canLoadSession,
      retry: false,
    }
  )
  const restoredThreadId = sessionThreads?.[0]?.id ?? null

  const {
    data: threadMessages,
    isLoading: isThreadMessagesLoading,
  } = api.useQuery(
    'get',
    '/v1/agents/{id}/messages',
    {
      params: {
        path: {
          id: restoredThreadId ?? '',
        },
      },
    },
    {
      enabled: Boolean(restoredThreadId),
      retry: false,
    }
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
      enabled: canLoadSession && !restoredThreadId && !isThreadsLoading,
      retry: false,
    }
  )

  const spawnMutation = api.useMutation('post', '/v1/agents/spawn')
  const inputMutation = api.useMutation('post', '/v1/agents/{id}/input')
  const approveMutation = api.useMutation('post', '/v1/agents/{id}/approve')
  const interruptMutation = api.useMutation('post', '/v1/agents/{id}/interrupt')

  const isRequesting =
    isBusyStatus(status) ||
    spawnMutation.isPending ||
    inputMutation.isPending ||
    approveMutation.isPending
  const isHistoryLoading =
    isThreadsLoading ||
    (restoredThreadId ? isThreadMessagesLoading : isSessionMessagesLoading)

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
    setMessages((current) => [
      ...current,
      {
        id: nextId('assistant'),
        message: {
          role: 'assistant',
          content,
        },
        status: 'error',
      },
    ])
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
            title: `${event.tool_name} approval`,
            toolName: event.tool_name,
          })
          break
        case 'assistant_delta':
          appendAssistantDelta(event.text)
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
            title: `${event.tool_name} started`,
            toolName: event.tool_name,
          })
          break
        case 'turn_completed':
          setStatus('completed')
          setPendingApproval(null)
          setThoughts((current) =>
            current.map((thought) =>
              thought.status === 'loading' ? { ...thought, status: 'success' } : thought
            )
          )
          completeAssistantTurn(event.text)
          void refetchSessionThreads()
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
      completeAssistantTurn,
      locale.eventStreamLagged,
      refetchSessionThreads,
      replaceThought,
      updateThoughtStatus,
    ]
  )

  useEffect(() => {
    if (sessionRef.current === resolvedSessionId) {
      return
    }

    sessionRef.current = resolvedSessionId
    eventSourceRef.current?.close()
    eventSourceRef.current = null
    seenEventIdsRef.current.clear()
    setActiveConversation(undefined)
    setMessages([])
    setThreadId(null)
    setStatus(null)
    setThoughts([])
    setPendingApproval(null)
    setEventsConnected(false)
  }, [resolvedSessionId])

  useEffect(() => {
    if (isRequesting || isHistoryLoading) {
      return
    }

    if (restoredThreadId) {
      setThreadId(restoredThreadId)
      setMessages(projectAgentThreadMessages(threadMessages))
      setStatus(sessionThreads?.[0]?.status ?? null)
      return
    }

    setThreadId(null)
    setMessages(projectSessionMessages(sessionMessages))
    setStatus(null)
  }, [
    isHistoryLoading,
    isRequesting,
    restoredThreadId,
    sessionMessages,
    sessionThreads,
    threadMessages,
  ])

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
    source.addEventListener('open', handleOpen)
    source.addEventListener('error', handleError)
    source.addEventListener('message', handleMessage)

    return () => {
      source.removeEventListener('open', handleOpen)
      source.removeEventListener('error', handleError)
      source.removeEventListener('message', handleMessage)
      source.close()
      if (eventSourceRef.current === source) {
        eventSourceRef.current = null
      }
      setEventsConnected(false)
    }
  }, [handleAgentEvent, threadId])

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

      setMessages((current) => [...current, userMessage])
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
          const response = await spawnMutation.mutateAsync({
            body: {
              config: toAgentConfig(model, runtimePresets),
              messages: requestMessages,
              session_id: resolvedSessionId,
            },
          })

          setThreadId(response.thread_id)
          return
        }

        await inputMutation.mutateAsync({
          body: {
            content: prompt,
          },
          params: {
            path: {
              id: threadId,
            },
          },
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
      inputMutation,
      isRequesting,
      locale.requestFailed,
      messages,
      model,
      resolvedSessionId,
      runtimePresets,
      spawnMutation,
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
        const response = await approveMutation.mutateAsync({
          body: {
            approved,
            call_id: decision.callId,
          },
          params: {
            path: {
              id: threadId,
            },
          },
        })

        if (!response.delivered) {
          toast.error(locale.approvalNotDelivered)
        }
      } catch (error) {
        toast.error(locale.approvalFailed, {
          description: getErrorMessage(error),
        })
      }
    },
    [
      approveMutation,
      locale.approvalFailed,
      locale.approvalNotDelivered,
      pendingApproval,
      threadId,
      updateThoughtStatus,
    ]
  )

  const abort = useCallback(() => {
    if (!threadId || !isRequesting) {
      return
    }

    void interruptMutation
      .mutateAsync({
        params: {
          path: {
            id: threadId,
          },
        },
      })
      .then(() => {
        setStatus('shutdown')
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
  }, [interruptMutation, isRequesting, locale.interruptFailed, threadId])

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
