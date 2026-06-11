import { SERVER_BASE_URL } from '@slab/api/config'

import type {
  AgentResponsesServerMessage,
  AgentStatus,
  AssistantMessageRecord,
  AssistantRuntimePresets,
  AssistantThought,
} from '../assistant-context'

export function nextId(prefix: string) {
  const random =
    typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
      ? crypto.randomUUID()
      : `${Date.now()}-${Math.random().toString(36).slice(2)}`

  return `${prefix}-${random}`
}

export function isBusyStatus(status: AgentStatus | null) {
  return status === 'pending' || status === 'running' || status === 'interrupting'
}

export function toAgentConfig(
  model: string,
  runtimePresets?: AssistantRuntimePresets | null,
  deepThink?: boolean
) {
  return {
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

export function updateLastAssistantMessage(
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

export function withThoughts(
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

export function agentResponsesWebSocketUrl() {
  const endpoint = new URL(SERVER_BASE_URL)
  endpoint.protocol = endpoint.protocol === 'https:' ? 'wss:' : 'ws:'
  endpoint.pathname = '/v1/agents/responses'
  endpoint.search = ''
  endpoint.hash = ''
  return endpoint.toString()
}

export function agentResponsesSseUrl(threadId: string) {
  const endpoint = new URL(SERVER_BASE_URL)
  endpoint.pathname = '/v1/agents/responses'
  endpoint.search = ''
  endpoint.searchParams.set('transport', 'sse')
  endpoint.searchParams.set('thread_id', threadId)
  endpoint.hash = ''
  return endpoint.toString()
}

export function agentEventKey(data: string): string | null {
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

export function serverMessageThreadId(message: AgentResponsesServerMessage): string | null {
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
