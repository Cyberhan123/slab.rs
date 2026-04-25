import type { DefaultMessageInfo } from '@ant-design/x-sdk'

import { apiClient } from '@slab/api'

import {
  isEphemeralConversationKey,
  isRecord,
  type ChatUiMessage,
  type SessionMessageResponse,
} from './chat-types'

const isSessionMessageResponse = (value: unknown): value is SessionMessageResponse => {
  if (!isRecord(value)) {
    return false
  }

  return (
    typeof value.id === 'string' &&
    typeof value.session_id === 'string' &&
    typeof value.role === 'string' &&
    typeof value.content === 'string' &&
    typeof value.created_at === 'string'
  )
}

const isStoredSessionEnvelope = (
  value: unknown
): value is {
  message: {
    role?: unknown
    content?: unknown
    tool_call_id?: unknown
    tool_calls?: unknown
  }
} => isRecord(value) && isRecord(value.message)

const isStoredConversationMessage = (
  value: unknown
): value is {
  role?: unknown
  content?: unknown
  tool_call_id?: unknown
  tool_calls?: unknown
} => isRecord(value) && ('content' in value || 'tool_calls' in value || 'tool_call_id' in value)

const renderStoredToolCall = (value: unknown): string => {
  if (!isRecord(value) || !isRecord(value.function) || typeof value.function.name !== 'string') {
    return ''
  }

  const rawArguments = value.function.arguments
  const argumentsText = typeof rawArguments === 'string' ? rawArguments : JSON.stringify(rawArguments ?? '')
  const callId = typeof value.id === 'string' && value.id.trim() ? ` id=${value.id.trim()}` : ''

  return `tool_call${callId}: ${value.function.name}(${argumentsText})`
}

const renderStoredContentPart = (value: unknown): string => {
  if (!isRecord(value) || typeof value.type !== 'string') {
    return typeof value === 'string' ? value : JSON.stringify(value ?? '')
  }

  switch (value.type) {
    case 'text':
    case 'input_text':
    case 'output_text':
    case 'refusal':
      return typeof value.text === 'string' ? value.text : ''
    case 'json':
      return JSON.stringify(value.value ?? null)
    case 'tool_result': {
      const rendered = JSON.stringify(value.value ?? null)
      const prefix =
        typeof value.tool_call_id === 'string' && value.tool_call_id.trim()
          ? `tool_result[${value.tool_call_id.trim()}]`
          : 'tool_result'
      return `${prefix}: ${rendered}`
    }
    case 'image': {
      const mime =
        typeof value.mime_type === 'string' && value.mime_type.trim() ? value.mime_type.trim() : 'unknown'
      const source =
        typeof value.image_url === 'string' && value.image_url.trim() ? value.image_url.trim() : 'embedded'
      const detail =
        typeof value.detail === 'string' && value.detail.trim() ? ` detail=${value.detail.trim()}` : ''
      return `[image mime=${mime} src=${source}${detail}]`
    }
    default:
      return JSON.stringify(value)
  }
}

const renderStoredMessageContent = (content: unknown): string => {
  if (typeof content === 'string') {
    return content
  }

  if (isRecord(content) && typeof content.text === 'string') {
    return content.text
  }

  if (Array.isArray(content)) {
    return content
      .map(renderStoredContentPart)
      .filter((part) => part.trim().length > 0)
      .join('\n')
  }

  if (content === undefined || content === null) {
    return ''
  }

  return JSON.stringify(content)
}

const toStoredSessionChatMessage = (
  fallbackRole: string,
  content: string
): Pick<ChatUiMessage, 'content' | 'role'> => {
  const payload = JSON.parse(content) as unknown
  const message = isStoredSessionEnvelope(payload)
    ? payload.message
    : isStoredConversationMessage(payload)
      ? payload
      : null

  if (!message) {
    return {
      content,
      role: fallbackRole as ChatUiMessage['role'],
    }
  }

  const segments: string[] = []
  const body = renderStoredMessageContent(message.content)
  if (body.trim()) {
    segments.push(body)
  }

  if (typeof message.tool_call_id === 'string' && message.tool_call_id.trim()) {
    segments.push(`tool_call_id: ${message.tool_call_id.trim()}`)
  }

  if (Array.isArray(message.tool_calls)) {
    const toolCalls = message.tool_calls
      .map(renderStoredToolCall)
      .filter((part) => part.trim().length > 0)
    if (toolCalls.length > 0) {
      segments.push(toolCalls.join('\n'))
    }
  }

  return {
    content: segments.join('\n'),
    role:
      (typeof message.role === 'string' && message.role.trim() ? message.role.trim() : fallbackRole) as ChatUiMessage['role'],
  }
}

const fetchSessionMessages = async (conversationKey?: string): Promise<SessionMessageResponse[]> => {
  if (isEphemeralConversationKey(conversationKey)) {
    return []
  }

  try {
    const { data, response } = await apiClient.GET('/v1/sessions/{id}/messages', {
      params: {
        path: {
          id: conversationKey ?? '',
        },
      },
    })

    if (response.status === 404) {
      return []
    }

    return Array.isArray(data)
      ? data.filter((item): item is SessionMessageResponse => isSessionMessageResponse(item))
      : []
  } catch (error) {
    console.warn('failed to load session messages', { conversationKey, error })
    return []
  }
}

export const historyMessageFactory = async (info?: {
  conversationKey?: string | number
}): Promise<DefaultMessageInfo<ChatUiMessage>[]> => {
  const conversationKey = typeof info?.conversationKey === 'number' ? String(info.conversationKey) : info?.conversationKey
  const messages = await fetchSessionMessages(conversationKey)

  return messages.map((message) => ({
    id: message.id,
    message: (() => {
      try {
        return toStoredSessionChatMessage(message.role, message.content)
      } catch {
        return {
          role: message.role as ChatUiMessage['role'],
          content: message.content,
        }
      }
    })(),
  }))
}
