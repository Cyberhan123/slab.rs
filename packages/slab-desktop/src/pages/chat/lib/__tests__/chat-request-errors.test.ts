import { describe, expect, it } from 'vitest'

import { ChatTransportError, getChatErrorDescription } from '../chat-request-errors'

describe('chat request errors', () => {
  it('uses typed chat transport messages', () => {
    expect(
      getChatErrorDescription(
        new ChatTransportError({
          message: 'model failed',
          transport_status: 500,
        }),
        'fallback'
      )
    ).toBe('model failed')
  })

  it('uses generic error messages before fallback text', () => {
    expect(getChatErrorDescription(new Error('network failed'), 'fallback')).toBe('network failed')
    expect(getChatErrorDescription({ message: 'bad request' }, 'fallback')).toBe('bad request')
    expect(getChatErrorDescription({ error: 'missing model' }, 'fallback')).toBe('missing model')
  })

  it('falls back when no message-like value is present', () => {
    expect(getChatErrorDescription({ message: '   ', error: '' }, 'fallback')).toBe('fallback')
    expect(getChatErrorDescription(null, 'fallback')).toBe('fallback')
  })
})
