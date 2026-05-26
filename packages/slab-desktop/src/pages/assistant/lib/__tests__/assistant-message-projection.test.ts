import { describe, expect, it } from 'vitest'

import { projectAgentThreadMessages, projectSessionMessages } from '../assistant-message-projection'

describe('assistant message projection', () => {
  it('projects legacy session messages', () => {
    expect(
      projectSessionMessages([
        {
          content: 'hello',
          created_at: '2026-01-01T00:00:00Z',
          id: 'msg-1',
          role: 'user',
          session_id: 'session-1',
        },
      ])
    ).toEqual([
      {
        id: 'msg-1',
        message: {
          content: 'hello',
          role: 'user',
        },
        status: 'success',
      },
    ])
  })

  it('projects persisted agent thread messages and hides tool-only turns', () => {
    expect(
      projectAgentThreadMessages([
        {
          content: 'request',
          created_at: '2026-01-01T00:00:00Z',
          id: 'msg-1',
          role: 'user',
          thread_id: 'thread-1',
          turn_index: 0,
        },
        {
          content: 'tool output',
          created_at: '2026-01-01T00:00:01Z',
          id: 'msg-2',
          role: 'tool',
          thread_id: 'thread-1',
          turn_index: 0,
        },
        {
          content: 'answer',
          created_at: '2026-01-01T00:00:02Z',
          id: 'msg-3',
          role: 'assistant',
          thread_id: 'thread-1',
          turn_index: 1,
        },
      ])
    ).toEqual([
      {
        id: 'msg-1',
        message: {
          content: 'request',
          role: 'user',
        },
        status: 'success',
      },
      {
        id: 'msg-3',
        message: {
          content: 'answer',
          role: 'assistant',
        },
        status: 'success',
      },
    ])
  })
})
