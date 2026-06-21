import { SERVER_BASE_URL } from '@slab/api/config'
import type { components } from '@slab/api'

import type {
  AgentResponsesServerMessage,
  AgentStatus,
  AssistantMessageRecord,
  AssistantRuntimePresets,
  AssistantThought,
} from '../assistant-context'

type AgentConfigInput = components['schemas']['AgentConfigInput']
export type AssistantReasoningEffort = components['schemas']['ChatReasoningEffort']
export type AssistantToolChoice = components['schemas']['AgentToolChoiceInput']

export type AssistantSlashCommandName = 'plan' | 'skill' | 'mcp' | 'web_search'

export type AssistantSlashCommandConfig = {
  command: AssistantSlashCommandName
  content: string
  allowedTools: string[]
  toolChoice: AssistantToolChoice
}

type ToAgentConfigOptions = {
  model: string
  runtimePresets?: AssistantRuntimePresets | null
  reasoningEffort?: AssistantReasoningEffort | null
  reasoningSupported?: boolean
  systemPrompt?: string | null
  toolConcurrency?: number | null
  toolChoice?: AssistantToolChoice | null
  slashCommand?: AssistantSlashCommandConfig | null
}

const SLASH_COMMAND_TOOL_CONFIG: Record<
  AssistantSlashCommandName,
  Omit<AssistantSlashCommandConfig, 'command' | 'content'>
> = {
  mcp: {
    allowedTools: ['mcp_list_tools', 'mcp_call'],
    toolChoice: { type: 'required' },
  },
  plan: {
    allowedTools: ['plan_update'],
    toolChoice: { name: 'plan_update', type: 'tool' },
  },
  skill: {
    allowedTools: ['delegate_subagent'],
    toolChoice: { name: 'delegate_subagent', type: 'tool' },
  },
  web_search: {
    allowedTools: ['web_search'],
    toolChoice: { name: 'web_search', type: 'tool' },
  },
}

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

export function parseAssistantSlashCommand(value: string): AssistantSlashCommandConfig | null {
  const match = value.trim().match(/^\/(plan|skill|mcp|web_search)(?:\s+([\s\S]*))?$/i)
  if (!match) {
    return null
  }

  const command = match[1].toLowerCase() as AssistantSlashCommandName
  const config = SLASH_COMMAND_TOOL_CONFIG[command]

  return {
    command,
    content: match[2]?.trim() ?? '',
    allowedTools: config.allowedTools,
    toolChoice: config.toolChoice,
  }
}

export function toAgentConfig({
  model,
  runtimePresets,
  reasoningEffort,
  reasoningSupported = true,
  systemPrompt,
  toolConcurrency,
  toolChoice,
  slashCommand,
}: ToAgentConfigOptions): AgentConfigInput {
  const trimmedSystemPrompt = systemPrompt?.trim()
  const selectedToolChoice = slashCommand?.toolChoice ?? toolChoice
  const allowedTools = slashCommand?.allowedTools ?? []

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
    ...(reasoningSupported && reasoningEffort ? { reasoning_effort: reasoningEffort } : {}),
    ...(trimmedSystemPrompt ? { system_prompt: trimmedSystemPrompt } : {}),
    ...(toolConcurrency && toolConcurrency > 1
      ? { tool_concurrency: Math.min(4, Math.max(1, Math.trunc(toolConcurrency))) }
      : {}),
    ...(selectedToolChoice && selectedToolChoice.type !== 'auto'
      ? { tool_choice: selectedToolChoice }
      : {}),
    ...(allowedTools.length > 0 ? { allowed_tools: allowedTools } : {}),
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
