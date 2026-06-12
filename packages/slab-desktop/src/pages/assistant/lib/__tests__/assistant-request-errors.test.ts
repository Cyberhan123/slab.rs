import { describe, expect, it } from 'vitest'

import {
  AssistantTransportError,
  adaptAssistantTransportResponse,
  extractStreamChunkError,
  getAssistantErrorDescription,
  getAssistantRequestErrorMessage,
  getAssistantRequestErrorMeta,
  getResponseRequestId,
  isAssistantRequestErrorInfo,
  isAssistantTransportError,
} from '../assistant-request-errors'

const translate = (key: string, options?: Record<string, unknown>) => {
  if (key === 'server.errors.badRequest') {
    return `translated ${options?.detail}`
  }

  return typeof options?.defaultValue === 'string' ? options.defaultValue : key
}

describe('assistant request errors', () => {
  it('uses typed assistant transport messages', () => {
    const error = new AssistantTransportError({
      message: 'model failed',
      transport_status: 500,
      code: 'runtime_error',
      param: 'model',
      request_id: 'req-1',
      error_type: 'server_error',
    })

    expect(isAssistantTransportError(error)).toBe(true)
    expect(getAssistantErrorDescription(error, 'fallback')).toBe('model failed')
    expect(getAssistantRequestErrorMessage(error)).toBe('model failed')
    expect(getAssistantRequestErrorMeta(error)).toEqual({
      errorCode: 'runtime_error',
      errorParam: 'model',
      errorStatus: 500,
      errorType: 'server_error',
    })
  })

  it('translates assistant transport i18n payloads with legacy fallback', () => {
    const error = new AssistantTransportError({
      message: 'model is required',
      transport_status: 400,
      error_type: 'invalid_request_error',
      i18n: {
        message: {
          key: 'server.errors.badRequest',
          params: { detail: 'model is required' },
        },
      },
    })

    expect(getAssistantErrorDescription(error, 'fallback', translate)).toBe(
      'translated model is required',
    )

    const fallbackError = new AssistantTransportError({
      message: 'legacy message',
      transport_status: 400,
      error_type: 'invalid_request_error',
      i18n: {
        message: {
          key: 'server.errors.conflict',
        },
      },
    })

    expect(getAssistantErrorDescription(fallbackError, 'fallback', translate)).toBe('legacy message')
  })

  it('uses generic error messages before fallback text', () => {
    expect(getAssistantErrorDescription(new Error('network failed'), 'fallback')).toBe('network failed')
    expect(getAssistantErrorDescription({ message: 'bad request' }, 'fallback')).toBe('bad request')
    expect(getAssistantErrorDescription({ error: 'missing model' }, 'fallback')).toBe('missing model')
  })

  it('falls back when no message-like value is present', () => {
    expect(getAssistantErrorDescription({ message: '   ', error: '' }, 'fallback')).toBe('fallback')
    expect(getAssistantErrorDescription(null, 'fallback')).toBe('fallback')
  })

  it('accepts structured assistant request error payloads', () => {
    const errorInfo = {
      success: false,
      message: 'request failed',
      name: 'ApiError',
      status: 400,
      statusText: 'Bad Request',
      error: {
        message: 'model is required',
        type: 'invalid_request_error',
        code: null,
        param: 'model',
      },
    }

    expect(isAssistantRequestErrorInfo(errorInfo)).toBe(true)
    expect(getAssistantRequestErrorMessage(errorInfo)).toBe('model is required')
    expect(getAssistantRequestErrorMeta(errorInfo)).toEqual({
      errorCode: undefined,
      errorParam: 'model',
      errorStatus: 400,
      errorType: 'invalid_request_error',
    })
  })

  it.each([
    null,
    {},
    { success: true, error: {} },
    { success: false, error: {}, message: 'bad', name: 'ApiError', status: 400, statusText: 'Bad Request' },
    {
      success: false,
      error: { message: 'bad', type: 'invalid_request_error', code: 42 },
      message: 'bad',
      name: 'ApiError',
      status: 400,
      statusText: 'Bad Request',
    },
    {
      success: false,
      error: { message: 'bad', type: 'invalid_request_error', param: { field: 'model' } },
      message: 'bad',
      name: 'ApiError',
      status: 400,
      statusText: 'Bad Request',
    },
    {
      success: false,
      error: { message: 'bad', type: 'invalid_request_error' },
      message: 'bad',
      name: 'ApiError',
      status: '400',
      statusText: 'Bad Request',
    },
  ])('rejects malformed assistant request error payload %#', (payload) => {
    expect(isAssistantRequestErrorInfo(payload)).toBe(false)
    expect(getAssistantRequestErrorMessage(payload)).toBeUndefined()
    expect(getAssistantRequestErrorMeta(payload)).toEqual({})
  })

  it('extracts request ids from supported response headers', () => {
    expect(
      getResponseRequestId(
        new Response('', {
          headers: { 'x-request-id': ' req-primary ' },
        })
      )
    ).toBe('req-primary')
    expect(getResponseRequestId(new Headers({ 'x-requestid': 'req-legacy' }))).toBe('req-legacy')
    expect(getResponseRequestId(new Headers({ 'x-trace-id': ' trace-1 ' }))).toBe('trace-1')
    expect(getResponseRequestId(new Headers({ 'x-request-id': '   ' }))).toBeNull()
    expect(getResponseRequestId(null)).toBeNull()
  })

  it('extracts API errors from stream chunks', () => {
    const error = {
      message: 'stream failed',
      type: 'server_error',
      code: 'runtime_error',
      param: null,
    }

    expect(extractStreamChunkError({ data: JSON.stringify({ error }) } as never)).toEqual(error)
    expect(extractStreamChunkError({ data: { error } } as never)).toEqual(error)
    expect(extractStreamChunkError({ data: '[DONE]' } as never)).toBeNull()
    expect(extractStreamChunkError({ data: '{"error":' } as never)).toBeNull()
    expect(extractStreamChunkError({ data: JSON.stringify({ error: { ...error, code: 42 } }) } as never)).toBeNull()
  })

  it('passes through successful transport responses', async () => {
    const response = new Response('ok', { status: 200 })

    await expect(adaptAssistantTransportResponse(response)).resolves.toBe(response)
  })

  it('throws typed transport errors for structured JSON API failures', async () => {
    const response = new Response(
      JSON.stringify({
        error: {
          message: 'model is not available',
          type: 'invalid_request_error',
          code: 'model_not_found',
          param: 'model',
        },
      }),
      {
        status: 404,
        statusText: 'Not Found',
        headers: {
          'content-type': 'application/json; charset=utf-8',
          'x-request-id': ' req-404 ',
        },
      }
    )

    await expect(adaptAssistantTransportResponse(response)).rejects.toMatchObject({
      code: 'model_not_found',
      error_type: 'invalid_request_error',
      message: 'model is not available',
      param: 'model',
      request_id: 'req-404',
      transport_status: 404,
    })
  })

  it('falls back to text, status text, or HTTP status for unstructured transport failures', async () => {
    await expect(
      adaptAssistantTransportResponse(
        new Response('  upstream failed  ', {
          status: 502,
          statusText: 'Bad Gateway',
        })
      )
    ).rejects.toMatchObject({
      message: 'upstream failed',
      transport_status: 502,
    })

    await expect(
      adaptAssistantTransportResponse(
        new Response('', {
          status: 504,
          statusText: 'Gateway Timeout',
          headers: { 'x-trace-id': 'trace-504' },
        })
      )
    ).rejects.toMatchObject({
      message: 'Gateway Timeout',
      request_id: 'trace-504',
      transport_status: 504,
    })

    await expect(
      adaptAssistantTransportResponse(
        new Response('', {
          status: 599,
        })
      )
    ).rejects.toMatchObject({
      message: 'HTTP 599',
      transport_status: 599,
    })
  })
})
