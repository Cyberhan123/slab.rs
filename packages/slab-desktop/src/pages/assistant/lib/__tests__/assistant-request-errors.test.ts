import { describe, expect, it } from 'vitest'

import { AssistantTransportError, getAssistantErrorDescription } from '../assistant-request-errors'

describe('assistant request errors', () => {
  it('uses typed assistant transport messages', () => {
    expect(
      getAssistantErrorDescription(
        new AssistantTransportError({
          message: 'model failed',
          transport_status: 500,
        }),
        'fallback'
      )
    ).toBe('model failed')
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
})
