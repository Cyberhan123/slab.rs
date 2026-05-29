import { describe, expect, it } from 'vitest'

import { toStoredSessionAssistantMessage } from '../assistant-history'

describe('assistant history', () => {
  it('keeps plain stored content when it is not JSON', () => {
    expect(toStoredSessionAssistantMessage(' user ', 'plain text')).toEqual({
      role: 'user',
      content: 'plain text',
    })
  })

  it('strips trailing assistant tool artifacts from plain stored content', () => {
    expect(
      toStoredSessionAssistantMessage(
        ' assistant ',
        'answer\ntool_call id=call-1: web_search({"query":"Tokyo weather"})'
      )
    ).toEqual({
      role: 'assistant',
      content: 'answer',
    })
  })

  it('renders stored message envelopes with content parts and tool calls', () => {
    const stored = {
      message: {
        role: ' assistant ',
        content: [
          { type: 'text', text: 'hello' },
          { type: 'json', value: { ok: true } },
          { type: 'tool_result', tool_call_id: 'call-1', value: { value: 42 } },
        ],
        tool_call_id: 'call-1',
        tool_calls: [
          {
            id: 'call-2',
            function: {
              name: 'lookup',
              arguments: { query: 'slab' },
            },
          },
        ],
      },
    }

    expect(toStoredSessionAssistantMessage('user', JSON.stringify(stored))).toEqual({
      role: 'assistant',
      content: [
        'hello',
        '{"ok":true}',
        'tool_result[call-1]: {"value":42}',
      ].join('\n'),
    })
  })

  it('falls back to the response role when a stored message has no role', () => {
    expect(
      toStoredSessionAssistantMessage(
        ' assistant ',
        JSON.stringify({
          content: { text: 'from storage' },
        })
      )
    ).toEqual({
      role: 'assistant',
      content: 'from storage',
    })
  })

  it('preserves unknown stored roles after trimming', () => {
    expect(
      toStoredSessionAssistantMessage(
        'user',
        JSON.stringify({
          role: ' critic ',
          content: 'review',
        })
      )
    ).toEqual({
      role: 'critic',
      content: 'review',
    })
  })
})
