import { describe, expect, it } from 'vitest'

import { parseAssistantAgentStreamEvent } from '../assistant-agent-events'

describe('assistant agent SSE parser', () => {
  it.each([
    [
      'agent_status',
      '{"type":"agent_status","status":"running"}',
      { type: 'agent_status', status: 'running' },
    ],
    [
      'tool_call_started',
      '{"type":"tool_call_started","tool_name":"shell","call_id":"call-1","arguments":"{}"}',
      { type: 'tool_call_started', tool_name: 'shell', call_id: 'call-1', arguments: '{}' },
    ],
    [
      'tool_call_output',
      '{"type":"tool_call_output","call_id":"call-1","output":"ok"}',
      { type: 'tool_call_output', call_id: 'call-1', output: 'ok' },
    ],
    [
      'approval_required',
      '{"type":"approval_required","call_id":"call-1","tool_name":"shell","command":"pwd"}',
      { type: 'approval_required', call_id: 'call-1', tool_name: 'shell', command: 'pwd' },
    ],
    [
      'turn_completed',
      '{"type":"turn_completed","text":"done"}',
      { type: 'turn_completed', text: 'done' },
    ],
    [
      'turn_failed',
      '{"type":"turn_failed","error":"failed"}',
      { type: 'turn_failed', error: 'failed' },
    ],
  ])('parses %s', (_, raw, expected) => {
    expect(parseAssistantAgentStreamEvent(raw)).toEqual(expected)
  })

  it('ignores malformed events', () => {
    expect(parseAssistantAgentStreamEvent('not json')).toBeNull()
    expect(parseAssistantAgentStreamEvent('{"type":"tool_call_output"}')).toBeNull()
  })
})
