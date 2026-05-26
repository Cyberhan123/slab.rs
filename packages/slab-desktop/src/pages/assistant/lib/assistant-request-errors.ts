import type { SSEFields, XModelResponse } from '@ant-design/x-sdk'

import { getErrorDescription } from '@/lib/error-description'

import { extractChunkPayload } from './assistant-message-utils'
import {
  isRecord,
  type AssistantApiError,
  type AssistantApiErrorResponse,
  type AssistantRequestErrorInfo,
  type AssistantRequestErrorType,
  type AssistantUiMessage,
} from './assistant-types'

export class AssistantTransportError extends Error {
  readonly transport_status: number
  readonly code?: AssistantApiError['code']
  readonly param?: AssistantApiError['param']
  readonly request_id?: string | null
  readonly error_type?: AssistantRequestErrorType

  constructor(options: {
    message: string
    transport_status: number
    code?: AssistantApiError['code']
    param?: AssistantApiError['param']
    request_id?: string | null
    error_type?: AssistantRequestErrorType
  }) {
    super(options.message)
    this.name = 'AssistantTransportError'
    this.transport_status = options.transport_status
    this.code = options.code
    this.param = options.param
    this.request_id = options.request_id
    this.error_type = options.error_type
  }
}

const isAssistantApiErrorResponse = (value: unknown): value is AssistantApiErrorResponse => {
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

export const isAssistantRequestErrorInfo = (value: unknown): value is AssistantRequestErrorInfo => {
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

export const isAssistantTransportError = (value: unknown): value is AssistantTransportError =>
  value instanceof AssistantTransportError

export const getAssistantRequestErrorMessage = (value: unknown): string | undefined => {
  if (isAssistantTransportError(value)) {
    return value.message
  }

  return isAssistantRequestErrorInfo(value) ? value.error.message : undefined
}

export const getAssistantErrorDescription = (value: unknown, fallback: string): string => {
  const requestErrorMessage = getAssistantRequestErrorMessage(value)
  if (requestErrorMessage?.trim()) {
    return requestErrorMessage
  }

  return getErrorDescription(value, fallback)
}

export const getAssistantRequestErrorMeta = (
  value: unknown
): Pick<AssistantUiMessage, 'errorCode' | 'errorParam' | 'errorStatus' | 'errorType'> => {
  if (isAssistantTransportError(value)) {
    return {
      errorCode: value.code ?? undefined,
      errorParam: value.param ?? undefined,
      errorStatus: value.transport_status,
      errorType: value.error_type,
    }
  }

  if (!isAssistantRequestErrorInfo(value)) {
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
): AssistantApiError | null => {
  const payload = extractChunkPayload(chunk)
  return isAssistantApiErrorResponse(payload) ? payload.error : null
}

const buildAssistantTransportError = async (response: Response): Promise<AssistantTransportError> => {
  const contentType = response.headers.get('content-type') ?? ''
  const request_id = getResponseRequestId(response)

  if (contentType.includes('application/json')) {
    const payload = await response.clone().json().catch(() => null)
    if (isAssistantApiErrorResponse(payload)) {
      return new AssistantTransportError({
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
  return new AssistantTransportError({
    message: rawBody.trim() || response.statusText || `HTTP ${response.status}`,
    transport_status: response.status,
    request_id,
  })
}

export const adaptAssistantTransportResponse = async (response: Response): Promise<Response> => {
  if (response.ok) {
    return response
  }

  throw await buildAssistantTransportError(response)
}
