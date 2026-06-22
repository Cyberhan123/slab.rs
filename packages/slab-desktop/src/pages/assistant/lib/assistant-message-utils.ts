import type { SSEFields, XModelMessage } from '@ant-design/x-sdk'

import { isRecord, type AssistantAgentRequestMessage, type AssistantUiMessage } from './assistant-types'

type AssistantMessageTextContentTarget = {
  role: XModelMessage['role']
  content?: unknown
} & Omit<AssistantUiMessage, 'role' | 'content'>

export const getAssistantMessageTextContent = (
  message?: { content?: unknown } | null
): string => {
  const content = message?.content

  if (typeof content === 'string') {
    return content
  }

  if (isRecord(content) && typeof content.text === 'string') {
    return content.text
  }

  return ''
}

const assistantTurnArtifactLinePattern =
  /^\s*(?:tool_call(?:\s+id=[^:]+)?:\s*[A-Za-z0-9_.-]+\(.*\)|tool_call_id:\s*.+)\s*$/i

export const stripTrailingAssistantTurnArtifacts = (value: string): string => {
  const cleaned = value
    .replace(/<\|endoftext\|>\s*<\|im_start\|>[\s\S]*$/g, '')
    .replace(/<\|endoftext\|>\s*$/g, '')
    .replace(/<\|im_end\|>\s*$/g, '')
    .replace(/<\|eot_id\|>\s*$/g, '')

  const lines = cleaned.split(/\r?\n/)
  let end = lines.length

  while (end > 0 && !lines[end - 1]?.trim()) {
    end -= 1
  }

  const artifactEnd = end
  while (end > 0 && assistantTurnArtifactLinePattern.test(lines[end - 1] ?? '')) {
    end -= 1
  }

  if (end === artifactEnd) {
    return cleaned
  }

  while (end > 0 && !lines[end - 1]?.trim()) {
    end -= 1
  }

  return lines.slice(0, end).join('\n').trimEnd()
}

const hasMeaningfulAssistantRequestContent = (
  message?: Pick<XModelMessage, 'content'> | null
): boolean => getAssistantMessageTextContent(message).trim().length > 0

export const toAssistantRequestMessage = (
  message?: Pick<XModelMessage, 'role' | 'content'> | null
): AssistantAgentRequestMessage | null => {
  if (!message || !hasMeaningfulAssistantRequestContent(message)) {
    return null
  }

  const text =
    message.role === 'assistant'
      ? stripTrailingAssistantTurnArtifacts(getAssistantMessageTextContent(message))
      : getAssistantMessageTextContent(message)

  if (!text.trim()) {
    return null
  }

  return {
    role: message.role,
    content: text,
  }
}

export const toAssistantRequestMessages = (
  messages?: Array<Pick<XModelMessage, 'role' | 'content'> | null | undefined>
): AssistantAgentRequestMessage[] => {
  return (messages ?? [])
    .map(toAssistantRequestMessage)
    .filter((message): message is AssistantAgentRequestMessage => Boolean(message))
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
        ? stripTrailingAssistantTurnArtifacts(getAssistantMessageTextContent(message))
        : getAssistantMessageTextContent(message)
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

export const extractChunkPayload = (chunk: Partial<Record<SSEFields, unknown>> | undefined): unknown => {
  const chunkData = (chunk as { data?: unknown } | undefined)?.data
  return parseSsePayload(chunkData) ?? chunkData ?? parseSsePayload(chunk) ?? chunk
}

export const stripThinkTags = (value: string): string =>
  value.replace(/<think\b[^>]*>[\s\S]*?<\/think>/gi, '').trim()

export const extractSseDeltaTextField = (
  chunk: Partial<Record<SSEFields, unknown>> | undefined,
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

export const withAssistantMessageTextContent = (
  message: AssistantMessageTextContentTarget,
  text: string
): AssistantUiMessage => {
  if (typeof message.content === 'string') {
    return {
      ...message,
      content: text,
    }
  }

  if (isRecord(message.content)) {
    return {
      ...message,
      content: {
        ...message.content,
        text,
        type: typeof message.content.type === 'string' ? message.content.type : 'text',
      },
    }
  }

  return {
    ...message,
    content: text,
  }
}

export const withAssistantMessageReasoningContent = (
  message: AssistantUiMessage,
  reasoningContent?: string
): AssistantUiMessage => ({
  ...message,
  reasoningContent: reasoningContent?.trim() ? reasoningContent : undefined,
})
