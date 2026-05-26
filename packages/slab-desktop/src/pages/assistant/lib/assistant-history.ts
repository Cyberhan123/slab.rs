import {
  isRecord,
  type AssistantUiMessage,
} from './assistant-types'

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

const stringifyStoredValue = (value: unknown, fallback = ''): string => {
  try {
    const rendered = JSON.stringify(value)
    return typeof rendered === 'string' ? rendered : fallback
  } catch {
    return fallback
  }
}

const parseStoredSessionPayload = (content: string): unknown | null => {
  try {
    return JSON.parse(content) as unknown
  } catch {
    return null
  }
}

const normalizeStoredMessageRole = (role: unknown, fallbackRole: string): AssistantUiMessage['role'] => {
  if (typeof role === 'string' && role.trim()) {
    return role.trim()
  }

  return fallbackRole.trim() || 'assistant'
}

const renderStoredToolCall = (value: unknown): string => {
  if (!isRecord(value) || !isRecord(value.function) || typeof value.function.name !== 'string') {
    return ''
  }

  const rawArguments = value.function.arguments
  const argumentsText = typeof rawArguments === 'string' ? rawArguments : stringifyStoredValue(rawArguments ?? '')
  const callId = typeof value.id === 'string' && value.id.trim() ? ` id=${value.id.trim()}` : ''

  return `tool_call${callId}: ${value.function.name}(${argumentsText})`
}

const renderStoredContentPart = (value: unknown): string => {
  if (!isRecord(value) || typeof value.type !== 'string') {
    return typeof value === 'string' ? value : stringifyStoredValue(value ?? '')
  }

  switch (value.type) {
    case 'text':
    case 'input_text':
    case 'output_text':
    case 'refusal':
      return typeof value.text === 'string' ? value.text : ''
    case 'json':
      return stringifyStoredValue(value.value ?? null, 'null')
    case 'tool_result': {
      const rendered = stringifyStoredValue(value.value ?? null, 'null')
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
      return stringifyStoredValue(value)
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

  return stringifyStoredValue(content)
}

export const toStoredSessionAssistantMessage = (
  fallbackRole: string,
  content: string
): Pick<AssistantUiMessage, 'content' | 'role'> => {
  const payload = parseStoredSessionPayload(content)
  const message = isStoredSessionEnvelope(payload)
    ? payload.message
    : isStoredConversationMessage(payload)
      ? payload
      : null

  if (!message) {
    return {
      content,
      role: normalizeStoredMessageRole(null, fallbackRole),
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
    role: normalizeStoredMessageRole(message.role, fallbackRole),
  }
}
