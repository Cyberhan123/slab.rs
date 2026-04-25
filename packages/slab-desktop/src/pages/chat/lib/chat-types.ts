import type { MessageInfo, XModelMessage, XModelParams } from '@ant-design/x-sdk'

import type { components } from '@slab/api/v1'

export type ChatApiError = components['schemas']['OpenAiError']
export type ChatApiErrorResponse = components['schemas']['OpenAiErrorResponse']
export type SessionMessageResponse = components['schemas']['MessageResponse']

export type ChatRequestErrorType = ChatApiError['type']

export type ChatUiMessage = XModelMessage & {
  errorCode?: ChatApiError['code']
  errorParam?: ChatApiError['param']
  errorStatus?: number
  errorType?: ChatRequestErrorType
  reasoningContent?: string
}

export type ChatMessageRecord = MessageInfo<ChatUiMessage>

export type ChatRequestParams = XModelParams & {
  continue_generation?: boolean
  max_tokens?: number | null
  temperature?: number | null
  thinking?: {
    type: 'enabled' | 'disabled'
  }
  top_p?: number | null
  top_k?: number | null
  min_p?: number | null
  presence_penalty?: number | null
  repetition_penalty?: number | null
  userAction?: string
}

export type ChatRuntimePresets = {
  max_tokens?: number | null
  temperature?: number | null
  top_p?: number | null
  top_k?: number | null
  min_p?: number | null
  presence_penalty?: number | null
  repetition_penalty?: number | null
}

export type ChatRequestErrorInfo = {
  error: ChatApiError
  message: string
  name: string
  status: number
  statusText: string
  success: false
}

export const DEFAULT_CONVERSATION_KEY = '__pending_session__'

export const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null

export const isEphemeralConversationKey = (value?: string): boolean => {
  const key = value?.trim()
  return !key || key === DEFAULT_CONVERSATION_KEY
}
