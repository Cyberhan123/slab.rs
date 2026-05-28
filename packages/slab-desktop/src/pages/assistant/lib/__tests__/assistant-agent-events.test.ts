import { describe, expect, it } from 'vitest'

import {
  parseAssistantAgentServerMessage,
  parseAssistantAgentStreamEvent,
} from '../assistant-agent-events'

describe('assistant agent SSE parser', () => {
  it.each([
    [
      'agent_status',
      '{"thread_id":"thread-1","sequence_number":1,"type":"agent.status","status":"running"}',
      { type: 'agent_status', status: 'running' },
    ],
    [
      'tool_call_started',
      '{"thread_id":"thread-1","sequence_number":2,"type":"response.function_call_arguments.done","name":"shell","call_id":"call-1","arguments":"{}"}',
      { type: 'tool_call_started', tool_name: 'shell', call_id: 'call-1', arguments: '{}' },
    ],
    [
      'tool_call_output',
      '{"thread_id":"thread-1","sequence_number":3,"type":"response.tool_call.output","call_id":"call-1","output":"ok","status":"completed"}',
      { type: 'tool_call_output', call_id: 'call-1', output: 'ok' },
    ],
    [
      'approval_required',
      '{"thread_id":"thread-1","sequence_number":4,"type":"response.tool_call.approval_required","call_id":"call-1","tool_name":"shell","command":"pwd"}',
      { type: 'approval_required', call_id: 'call-1', tool_name: 'shell', command: 'pwd' },
    ],
    [
      'assistant_delta',
      '{"thread_id":"thread-1","sequence_number":5,"type":"response.output_text.delta","delta":"hel"}',
      { type: 'assistant_delta', text: 'hel' },
    ],
    [
      'turn_completed',
      '{"thread_id":"thread-1","sequence_number":6,"type":"response.output_text.done","text":"done"}',
      { type: 'turn_completed', text: 'done' },
    ],
    [
      'turn_finished',
      '{"thread_id":"thread-1","sequence_number":7,"type":"response.completed","response":{"id":"thread-1","status":"completed"},"text":"done"}',
      { type: 'turn_finished' },
    ],
    [
      'turn_failed',
      '{"thread_id":"thread-1","sequence_number":8,"type":"response.failed","error":"failed"}',
      { type: 'turn_failed', error: 'failed' },
    ],
    [
      'turn_cancelled',
      '{"thread_id":"thread-1","sequence_number":9,"type":"response.cancelled","reason":"interrupted"}',
      { type: 'turn_cancelled', reason: 'interrupted' },
    ],
  ])('parses %s', (_, raw, expected) => {
    expect(parseAssistantAgentStreamEvent(raw)).toEqual(expected)
  })

  it('ignores malformed events', () => {
    expect(parseAssistantAgentStreamEvent('not json')).toBeNull()
    expect(parseAssistantAgentStreamEvent('{"type":"response.tool_call.output"}')).toBeNull()
    expect(
      parseAssistantAgentStreamEvent(
        '{"thread_id":"thread-1","sequence_number":9,"type":"response.metrics","metrics":{"name":"turn","duration_ms":1}}'
      )
    ).toBeNull()
  })

  it('parses agent transport control messages', () => {
    expect(
      parseAssistantAgentServerMessage(
        '{"type":"agent.ack","action":"approval_resolve","accepted":false,"delivered":false,"thread_id":"thread-1"}'
      )
    ).toMatchObject({
      accepted: false,
      action: 'approval_resolve',
      delivered: false,
      thread_id: 'thread-1',
      type: 'agent.ack',
    })
    expect(
      parseAssistantAgentServerMessage(
        '{"type":"agent.session.restored","session_id":"session-1","messages":[]}'
      )
    ).toMatchObject({
      messages: [],
      session_id: 'session-1',
      type: 'agent.session.restored',
    })
    expect(parseAssistantAgentServerMessage('{"type":"response.output_text.delta"}')).toBeNull()
  })
})
