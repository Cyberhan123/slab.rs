import type { SSEFields, XModelMessage, XModelResponse } from '@ant-design/x-sdk'

import { isRecord, type ChatUiMessage } from './chat-types'

export const getChatMessageTextContent = (
  message?: Pick<XModelMessage, 'content'> | null
): string => {
  const content = message?.content

  if (typeof content === 'string') {
    return content
  }

  if (content && typeof content.text === 'string') {
    return content.text
  }

  return ''
}

export const stripTrailingAssistantTurnArtifacts = (value: string): string =>
  value
    .replace(/<\|endoftext\|>\s*<\|im_start\|>[\s\S]*$/g, '')
    .replace(/<\|endoftext\|>\s*$/g, '')
    .replace(/<\|im_end\|>\s*$/g, '')
    .replace(/<\|eot_id\|>\s*$/g, '')

const hasMeaningfulChatRequestContent = (
  message?: Pick<XModelMessage, 'content'> | null
): boolean => getChatMessageTextContent(message).trim().length > 0

export const toChatRequestMessage = (
  message?: Pick<XModelMessage, 'role' | 'content'> | null
): XModelMessage | null => {
  if (!message || !hasMeaningfulChatRequestContent(message)) {
    return null
  }

  const text =
    message.role === 'assistant'
      ? stripTrailingAssistantTurnArtifacts(getChatMessageTextContent(message))
      : getChatMessageTextContent(message)

  return {
    role: message.role,
    content: text,
  }
}

export const toChatRequestMessages = (
  messages?: Array<Pick<XModelMessage, 'role' | 'content'> | null | undefined>
): XModelMessage[] => {
  return (messages ?? [])
    .map(toChatRequestMessage)
    .filter((message): message is XModelMessage => Boolean(message))
}

export const getContinueGenerationPrefix = (
  messages?: Array<Pick<XModelMessage, 'role' | 'content'> | null | undefined>
): string => {
  for (let index = (messages?.length ?? 0) - 1; index >= 0; index -= 1) {
    const message = messages?.[index]
    if (!message) {
      continue
    }

    const content =
      message.role === 'assistant'
        ? stripTrailingAssistantTurnArtifacts(getChatMessageTextContent(message))
        : getChatMessageTextContent(message)
    if (!content.trim()) {
      continue
    }

    return message.role === 'assistant' ? content : ''
  }

  return ''
}

export const mergeContinuationContent = (prefix: string, generated: string): string => {
  if (!prefix) {
    return generated
  }

  if (!generated) {
    return prefix
  }

  const maxOverlap = Math.min(prefix.length, generated.length)
  for (let size = maxOverlap; size > 0; size -= 1) {
    if (prefix.slice(-size) === generated.slice(0, size)) {
      return `${prefix}${generated.slice(size)}`
    }
  }

  return `${prefix}${generated}`
}

const parseSsePayload = (value: unknown): unknown => {
  if (typeof value !== 'string') {
    return value
  }

  const trimmed = value.trim()
  if (!trimmed || trimmed === '[DONE]') {
    return null
  }
  if ((trimmed.startsWith('{') || trimmed.startsWith('[')) && trimmed.length > 1) {
    try {
      return JSON.parse(trimmed) as unknown
    } catch {
      return null
    }
  }

  return null
}

export const extractChunkPayload = (
  chunk: Partial<Record<SSEFields, XModelResponse>> | undefined
): unknown => {
  const chunkData = (chunk as { data?: unknown } | undefined)?.data
  return parseSsePayload(chunkData) ?? chunkData ?? parseSsePayload(chunk) ?? chunk
}

export const extractSseDeltaTextField = (
  chunk: Partial<Record<SSEFields, XModelResponse>> | undefined,
  field: string
): string => {
  const payload = extractChunkPayload(chunk)
  if (!isRecord(payload) || !Array.isArray(payload.choices)) {
    return ''
  }

  for (const choice of payload.choices) {
    if (!isRecord(choice) || !isRecord(choice.delta)) {
      continue
    }
    const value = choice.delta[field]
    if (typeof value === 'string' && value.length > 0) {
      return value
    }
  }

  return ''
}

export const withChatMessageTextContent = (
  message: ChatUiMessage,
  text: string
): ChatUiMessage => {
  if (typeof message.content === 'string') {
    return {
      ...message,
      content: text,
    }
  }

  if (message.content && typeof message.content === 'object') {
    return {
      ...message,
      content: {
        ...message.content,
        text,
        type: message.content.type ?? 'text',
      },
    }
  }

  return {
    ...message,
    content: text,
  }
}

export const withChatMessageReasoningContent = (
  message: ChatUiMessage,
  reasoningContent?: string
): ChatUiMessage => ({
  ...message,
  reasoningContent: reasoningContent?.trim() ? reasoningContent : undefined,
})
