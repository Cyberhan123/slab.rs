import type { SSEFields, XModelResponse } from '@ant-design/x-sdk'

import { extractChunkPayload } from './chat-message-utils'
import {
  isRecord,
  type ChatApiError,
  type ChatApiErrorResponse,
  type ChatRequestErrorInfo,
  type ChatRequestErrorType,
  type ChatUiMessage,
} from './chat-types'

export class ChatTransportError extends Error {
  readonly transport_status: number
  readonly code?: ChatApiError['code']
  readonly param?: ChatApiError['param']
  readonly request_id?: string | null
  readonly error_type?: ChatRequestErrorType

  constructor(options: {
    message: string
    transport_status: number
    code?: ChatApiError['code']
    param?: ChatApiError['param']
    request_id?: string | null
    error_type?: ChatRequestErrorType
  }) {
    super(options.message)
    this.name = 'ChatTransportError'
    this.transport_status = options.transport_status
    this.code = options.code
    this.param = options.param
    this.request_id = options.request_id
    this.error_type = options.error_type
  }
}

const isChatApiErrorResponse = (value: unknown): value is ChatApiErrorResponse => {
  if (!isRecord(value) || !isRecord(value.error)) {
    return false
  }

  const { error } = value

  return (
    typeof error.message === 'string' &&
    typeof error.type === 'string' &&
    (!('code' in error) || error.code === null || typeof error.code === 'string') &&
    (!('param' in error) || error.param === null || typeof error.param === 'string')
  )
}

export const isChatRequestErrorInfo = (value: unknown): value is ChatRequestErrorInfo => {
  if (!isRecord(value) || value.success !== false || !isRecord(value.error)) {
    return false
  }

  return (
    typeof value.message === 'string' &&
    typeof value.name === 'string' &&
    typeof value.status === 'number' &&
    typeof value.statusText === 'string' &&
    typeof value.error.message === 'string' &&
    typeof value.error.type === 'string' &&
    (!('code' in value.error) || value.error.code === null || typeof value.error.code === 'string') &&
    (!('param' in value.error) || value.error.param === null || typeof value.error.param === 'string')
  )
}

export const isChatTransportError = (value: unknown): value is ChatTransportError =>
  value instanceof ChatTransportError

export const getChatRequestErrorMessage = (value: unknown): string | undefined => {
  if (isChatTransportError(value)) {
    return value.message
  }

  return isChatRequestErrorInfo(value) ? value.error.message : undefined
}

export const getChatRequestErrorMeta = (
  value: unknown
): Pick<ChatUiMessage, 'errorCode' | 'errorParam' | 'errorStatus' | 'errorType'> => {
  if (isChatTransportError(value)) {
    return {
      errorCode: value.code ?? undefined,
      errorParam: value.param ?? undefined,
      errorStatus: value.transport_status,
      errorType: value.error_type,
    }
  }

  if (!isChatRequestErrorInfo(value)) {
    return {}
  }

  return {
    errorCode: value.error.code ?? undefined,
    errorParam: value.error.param ?? undefined,
    errorStatus: value.status,
    errorType: value.error.type,
  }
}

export const getResponseRequestId = (source?: Response | Headers | null): string | null => {
  const headers = source instanceof Response ? source.headers : source
  const requestId =
    headers?.get('x-request-id') ?? headers?.get('x-requestid') ?? headers?.get('x-trace-id')
  const trimmed = requestId?.trim()
  return trimmed ? trimmed : null
}

export const extractStreamChunkError = (
  chunk: Partial<Record<SSEFields, XModelResponse>> | undefined
): ChatApiError | null => {
  const payload = extractChunkPayload(chunk)
  return isChatApiErrorResponse(payload) ? payload.error : null
}

const buildChatTransportError = async (response: Response): Promise<ChatTransportError> => {
  const contentType = response.headers.get('content-type') ?? ''
  const request_id = getResponseRequestId(response)

  if (contentType.includes('application/json')) {
    const payload = await response.clone().json().catch(() => null)
    if (isChatApiErrorResponse(payload)) {
      return new ChatTransportError({
        message: payload.error.message,
        transport_status: response.status,
        code: payload.error.code ?? undefined,
        param: payload.error.param ?? undefined,
        request_id,
        error_type: payload.error.type,
      })
    }
  }

  const rawBody = await response.clone().text().catch(() => '')
  return new ChatTransportError({
    message: rawBody.trim() || response.statusText || `HTTP ${response.status}`,
    transport_status: response.status,
    request_id,
  })
}

export const adaptChatTransportResponse = async (response: Response): Promise<Response> => {
  if (response.ok) {
    return response
  }

  throw await buildChatTransportError(response)
}
