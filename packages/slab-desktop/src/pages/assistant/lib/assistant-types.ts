import type { MessageInfo, XModelMessage, XModelParams } from '@ant-design/x-sdk'

import type { components } from '@slab/api/v1'

export type AssistantApiError = components['schemas']['OpenAiError']
export type AssistantApiErrorResponse = components['schemas']['OpenAiErrorResponse']
export type SessionMessageResponse = components['schemas']['MessageResponse']
export type AgentStatus = components['schemas']['AgentStatusValue']
export type AgentThreadResponse = components['schemas']['AgentThreadResponse']
export type AgentThreadMessageResponse = components['schemas']['AgentThreadMessageResponse']
export type AssistantAgentRequestMessage = components['schemas']['MessageInput']
export type AgentResponsesClientMessage = components['schemas']['AgentResponsesClientMessage']
export type AgentResponsesServerMessage = components['schemas']['AgentResponsesServerMessage']

export type AssistantRequestErrorType = AssistantApiError['type']

export type AssistantThoughtStatus = 'abort' | 'error' | 'loading' | 'success'

export type AssistantThought = {
  id: string
  title: string
  detail?: string
  status: AssistantThoughtStatus
  summary?: string
  toolName?: string
  callId?: string
  pendingApproval?: {
    callId: string
    toolName: string
    command: string
  }
}

export type AssistantUiMessage = XModelMessage & {
  errorCode?: AssistantApiError['code']
  errorParam?: AssistantApiError['param']
  errorStatus?: number
  errorType?: AssistantRequestErrorType
  reasoningContent?: string
  thoughts?: AssistantThought[]
}

export type AssistantMessageRecord = MessageInfo<AssistantUiMessage>

export type AssistantRequestParams = XModelParams & {
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

export type AssistantRuntimePresets = {
  max_tokens?: number | null
  temperature?: number | null
  top_p?: number | null
  top_k?: number | null
  min_p?: number | null
  presence_penalty?: number | null
  repetition_penalty?: number | null
}

export type AssistantRequestErrorInfo = {
  error: AssistantApiError
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
