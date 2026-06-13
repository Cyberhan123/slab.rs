import { describe, expect, it } from 'vitest'

import { formatKnownToolResult, projectAgentThreadMessages } from '../assistant-message-projection'

describe('assistant message projection', () => {
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
          content: '',
          created_at: '2026-01-01T00:00:01Z',
          id: 'msg-2',
          role: 'assistant',
          thread_id: 'thread-1',
          turn_index: 0,
          tool_calls: [
            {
              id: 'call-0',
              type: 'function',
              function: {
                name: 'web_search',
                arguments: '{"query":"Japan weather"}',
              },
            },
          ],
        },
        {
          content: 'tool output',
          created_at: '2026-01-01T00:00:02Z',
          id: 'msg-3',
          role: 'tool',
          thread_id: 'thread-1',
          tool_call_id: 'call-0',
          turn_index: 1,
        },
        {
          content: 'answer',
          created_at: '2026-01-01T00:00:03Z',
          id: 'msg-4',
          role: 'assistant',
          thread_id: 'thread-1',
          turn_index: 2,
        },
        {
          content: 'tool_call id=call-1: web_search({"query":"Japan weather"})',
          created_at: '2026-01-01T00:00:04Z',
          id: 'msg-5',
          role: 'assistant',
          thread_id: 'thread-1',
          turn_index: 3,
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
        id: 'msg-2',
        message: {
          content: 'answer',
          role: 'assistant',
          thoughts: [
            {
              callId: 'call-0',
              detail: 'tool output',
              id: 'call-0',
              status: 'success',
              summary: 'tool_call id=call-0: web_search({"query":"Japan weather"})',
              title: 'tool_call',
              toolName: 'web_search',
            },
          ],
        },
        status: 'success',
      },
    ])
  })

  it('keeps pending restored tool calls loading while the thread is running', () => {
    expect(
      projectAgentThreadMessages(
        [
          {
            content: '',
            created_at: '2026-01-01T00:00:01Z',
            id: 'msg-1',
            role: 'assistant',
            thread_id: 'thread-1',
            turn_index: 0,
            tool_calls: [
              {
                id: 'call-0',
                type: 'function',
                function: {
                  name: 'web_search',
                  arguments: '{"query":"Japan weather"}',
                },
              },
            ],
          },
        ],
        'running'
      )
    ).toEqual([
      {
        id: 'msg-1',
        message: {
          content: '',
          role: 'assistant',
          thoughts: [
            {
              callId: 'call-0',
              detail: '{"query":"Japan weather"}',
              id: 'call-0',
              status: 'loading',
              summary: 'tool_call id=call-0: web_search({"query":"Japan weather"})',
              title: 'tool_call',
              toolName: 'web_search',
            },
          ],
        },
        status: 'success',
      },
    ])
  })

  it('formats known coding tool JSON results for replay', () => {
    expect(
      projectAgentThreadMessages([
        {
          content: '',
          created_at: '2026-01-01T00:00:01Z',
          id: 'msg-1',
          role: 'assistant',
          thread_id: 'thread-1',
          turn_index: 0,
          tool_calls: [
            {
              id: 'call-0',
              type: 'function',
              function: {
                name: 'plan_update',
                arguments: '{"items":[]}',
              },
            },
          ],
        },
        {
          content: JSON.stringify({
            summary: 'Route map',
            items: [
              { step: 'Inspect code', status: 'completed' },
              { step: 'Implement slice', status: 'in_progress' },
            ],
          }),
          created_at: '2026-01-01T00:00:02Z',
          id: 'msg-2',
          role: 'tool',
          thread_id: 'thread-1',
          tool_call_id: 'call-0',
          turn_index: 1,
        },
      ])
    ).toEqual([
      {
        id: 'msg-1',
        message: {
          content: '',
          role: 'assistant',
          thoughts: [
            {
              callId: 'call-0',
              detail: 'Route map\ncompleted: Inspect code\nin_progress: Implement slice',
              id: 'call-0',
              status: 'success',
              summary: 'tool_call id=call-0: plan_update({"items":[]})',
              title: 'tool_call',
              toolName: 'plan_update',
            },
          ],
        },
        status: 'success',
      },
    ])
  })

  it('formats lsp status tool results', () => {
    expect(
      formatKnownToolResult(
        'code_lsp_status',
        JSON.stringify({
          language_id: 'rust',
          provider: {
            id: 'builtin.rust-analyzer',
            transport: 'stdio',
          },
          workspace_root: 'C:\\repo',
        })
      )
    ).toBe('language: rust\nprovider: builtin.rust-analyzer (stdio)\nworkspace: C:\\repo')
  })
})
