import { SERVER_BASE_URL } from '@slab/api/config'
import { describe, expect, it } from 'vitest'

import type {
  AgentResponsesServerMessage,
  AssistantMessageRecord,
  AssistantThought,
} from '../assistant-types'
import {
  agentEventKey,
  agentResponsesSseUrl,
  agentResponsesWebSocketUrl,
  serverMessageThreadId,
  toAgentConfig,
  updateLastAssistantMessage,
  withThoughts,
} from '../assistant-agent-state'

function assistantMessage(
  id: string,
  status: AssistantMessageRecord['status'],
  content = ''
): AssistantMessageRecord {
  return {
    id,
    message: {
      content,
      role: 'assistant',
    },
    status,
  }
}

describe('assistant agent state helpers', () => {
  it('projects runtime presets and reasoning controls into the agent config', () => {
    expect(
      toAgentConfig(
        'qwen',
        {
          max_tokens: 2048,
          min_p: 0.2,
          presence_penalty: 0.4,
          repetition_penalty: 1.1,
          temperature: 0.6,
          top_k: 40,
          top_p: 0.9,
        },
        true
      )
    ).toEqual({
      max_tokens: 2048,
      max_turns: 8,
      min_p: 0.2,
      model: 'qwen',
      presence_penalty: 0.4,
      reasoning_effort: 'medium',
      repetition_penalty: 1.1,
      temperature: 0.6,
      top_k: 40,
      top_p: 0.9,
    })
    expect(toAgentConfig('qwen', { temperature: null }, false)).toEqual({
      max_turns: 8,
      model: 'qwen',
    })
  })

  it('updates the latest unfinished assistant message only', () => {
    const messages: AssistantMessageRecord[] = [
      {
        id: 'user-1',
        message: {
          content: 'hello',
          role: 'user',
        },
        status: 'success',
      },
      assistantMessage('assistant-success', 'success', 'done'),
      assistantMessage('assistant-loading', 'loading', 'draft'),
    ]

    expect(
      updateLastAssistantMessage(messages, (message) => ({
        ...message,
        message: {
          ...message.message,
          content: 'updated',
        },
        status: 'success',
      }))
    ).toEqual([
      messages[0],
      messages[1],
      assistantMessage('assistant-loading', 'success', 'updated'),
    ])
    expect(updateLastAssistantMessage([assistantMessage('assistant-success', 'success')], (message) => message)).toBeNull()
  })

  it('attaches thoughts to the latest assistant message or appends a loading shell', () => {
    const thoughts: AssistantThought[] = [
      {
        id: 'thought-1',
        status: 'loading',
        title: 'tool_call',
      },
    ]

    expect(
      withThoughts(
        [
          {
            id: 'user-1',
            message: {
              content: 'hello',
              role: 'user',
            },
            status: 'success',
          },
          assistantMessage('assistant-1', 'loading', 'draft'),
        ],
        thoughts
      )
    ).toEqual([
      {
        id: 'user-1',
        message: {
          content: 'hello',
          role: 'user',
        },
        status: 'success',
      },
      {
        id: 'assistant-1',
        message: {
          content: 'draft',
          role: 'assistant',
          thoughts,
        },
        status: 'loading',
      },
    ])

    const inserted = withThoughts(
      [
        {
          id: 'user-1',
          message: {
            content: 'hello',
            role: 'user',
          },
          status: 'success',
        },
      ],
      thoughts
    )
    expect(inserted).toHaveLength(2)
    expect(inserted[1]).toMatchObject({
      message: {
        content: '',
        role: 'assistant',
        thoughts,
      },
      status: 'loading',
    })
  })

  it('builds websocket and sse transport URLs from the server base URL', () => {
    expect(agentResponsesWebSocketUrl()).toBe(
      SERVER_BASE_URL.replace(/^http/, 'ws') + '/v1/agents/responses'
    )
    expect(agentResponsesSseUrl('thread-1')).toBe(
      `${SERVER_BASE_URL}/v1/agents/responses?transport=sse&thread_id=thread-1`
    )
  })

  it('extracts event keys and server message thread ids safely', () => {
    expect(agentEventKey('{"thread_id":"thread-1","sequence_number":2}')).toBe('thread-1:2')
    expect(agentEventKey('{"thread_id":"thread-1","sequence_number":"2"}')).toBeNull()
    expect(agentEventKey('not json')).toBeNull()

    expect(
      serverMessageThreadId({
        accepted: true,
        request_id: 'req-1',
        thread_id: 'thread-1',
        type: 'agent.ack',
      } as AgentResponsesServerMessage)
    ).toBe('thread-1')
    expect(
      serverMessageThreadId({
        code: 'server_error',
        message: 'boom',
        request_id: 'req-2',
        thread_id: 'thread-2',
        type: 'agent.error',
      } as AgentResponsesServerMessage)
    ).toBe('thread-2')
    expect(
      serverMessageThreadId({
        messages: [],
        request_id: 'req-3',
        session_id: 'session-1',
        thread: {
          completion_text: null,
          config_json: '{}',
          created_at: '2026-01-01T00:00:00Z',
          depth: 0,
          id: 'thread-3',
          session_id: 'session-1',
          status: 'completed',
          updated_at: '2026-01-01T00:00:00Z',
        },
        type: 'agent.session.restored',
      } as AgentResponsesServerMessage)
    ).toBe('thread-3')
  })
})
