import { render, screen } from '@testing-library/react'
import { StrictMode, useEffect, useState } from 'react'
import { describe, expect, it } from 'vitest'

import { Bubble, BubbleContent } from '@slab/components/bubble'
import { Message, MessageContent } from '@slab/components/message'
import {
  AssistantBubbleContentViewByContent,
  AssistantBubbleFooter,
  type AssistantBubbleContent,
} from '../assistant-bubble-content'
import { AssistantMarkdown } from '../message/markdown'

const bubbleLabels = {
  approve: 'Approve',
  assistant: 'Assistant',
  cancelEdit: 'Cancel edit',
  copy: 'Copy',
  edit: 'Edit',
  regenerate: 'Regenerate',
  reject: 'Reject',
  retry: 'Retry',
  saveEdit: 'Send edit',
  taskActionBlockedPath: 'Blocked path',
  taskActionFeedback: 'Follow up',
  taskActionOpen: 'Open',
  taskActionReview: 'Review',
  taskActionTitle: 'Artifacts ready',
  terminalCancelled: 'Generation was cancelled. Partial content was preserved.',
  thinkingLoading: 'Thinking...',
  thinkingReady: 'Reasoning trace',
  user: 'User',
  waitingForResponse: 'Waiting for response...',
}

function createAssistantBubbleContent(
  content: string,
  status: AssistantBubbleContent['item']['status'] = 'success'
): AssistantBubbleContent {
  return {
    approvingCallIds: [],
    item: {
      id: 'assistant-message',
      message: {
        content,
        role: 'assistant',
      },
      status,
    },
    labels: bubbleLabels,
  }
}

function StreamingMarkdown() {
  const [content, setContent] = useState('Searching')

  useEffect(() => {
    setContent('Searching\n\nCalling tools')
  }, [])

  return (
    <AssistantMarkdown className="assistant-markdown--assistant" hasNextChunk>
      {content}
    </AssistantMarkdown>
  )
}

describe('AssistantMarkdown', () => {
  it('renders latex, citations, and fenced code', () => {
    render(
      <AssistantMarkdown>
        {'Inline $x^2$ citation<sup><a href="https://example.com">1</a></sup>\n\n```diff\n+ added\n```'}
      </AssistantMarkdown>
    )

    expect(screen.getByText('1')).toBeInTheDocument()
    expect(screen.getByText('+ added')).toBeInTheDocument()
  })

  it('renders inside a shadcn Bubble without recursive updates', () => {
    render(
      <StrictMode>
        <Message>
          <MessageContent>
            <Bubble variant="ghost">
              <BubbleContent>
                <AssistantMarkdown className="assistant-markdown--assistant">
                  Help me check today&apos;s weather in Tokyo.
                </AssistantMarkdown>
              </BubbleContent>
            </Bubble>
          </MessageContent>
        </Message>
      </StrictMode>
    )

    expect(screen.getByText("Help me check today's weather in Tokyo.")).toBeInTheDocument()
  })

  it('renders streaming markdown content without recursive updates', () => {
    render(
      <StrictMode>
        <StreamingMarkdown />
      </StrictMode>
    )

    expect(screen.getByText('Searching')).toBeInTheDocument()
  })

  it('renders completed responses with think tags and long plain text without recursive updates', () => {
    const content =
      '<think status="done">\n\nThe assistant planned the answer.\n\n</think>\n\n' +
      'I can help with the following tasks:\n\n' +
      '1. **File operations** - read and write files\n' +
      '2. **Search** - search the codebase and the web\n\n' +
      'Since 2011, researchers have described learning sciences as an emerging field that studies human learning in natural settings. This paragraph intentionally looks like unrelated retrieved text so the renderer handles model output as ordinary content.\n\n' +
      'Reviews\n\n' +
      '"This book provides a comprehensive overview of the most recent research."'

    render(
      <StrictMode>
        <AssistantMarkdown className="assistant-markdown--assistant">
          {content}
        </AssistantMarkdown>
      </StrictMode>
    )

    expect(screen.getByText('I can help with the following tasks:')).toBeInTheDocument()
    expect(screen.queryByText('The assistant planned the answer.')).not.toBeInTheDocument()
    expect(
      screen.getByText(/This paragraph intentionally looks like unrelated retrieved text/)
    ).toBeInTheDocument()
  })

  it('routes think tags to the reasoning trace instead of duplicating them in the answer body', () => {
    render(
      <StrictMode>
        <AssistantBubbleContentViewByContent
          content={createAssistantBubbleContent(
            '<think status="done">chain-only-thinking</think>\n\nVisible answer body'
          )}
        />
      </StrictMode>
    )

    expect(screen.getByText('Visible answer body')).toBeInTheDocument()
    expect(screen.getAllByText('chain-only-thinking')).toHaveLength(1)
  })

  it('keeps streamed incomplete think content out of the answer body', () => {
    render(
      <StrictMode>
        <AssistantBubbleContentViewByContent
          content={createAssistantBubbleContent('<think>streaming-thinking', 'updating')}
        />
      </StrictMode>
    )

    expect(screen.getByText('streaming-thinking')).toBeInTheDocument()
    expect(screen.queryByText('<think>streaming-thinking')).not.toBeInTheDocument()
  })

  it('renders terminal error footer without replacing streamed content', () => {
    const content: AssistantBubbleContent = {
      ...createAssistantBubbleContent('Partial answer', 'error'),
      item: {
        id: 'assistant-message',
        message: {
          content: 'Partial answer',
          role: 'assistant',
          terminalNotice: {
            message: 'Network failed',
            type: 'error',
          },
        },
        status: 'error',
      },
      onRetry: () => {},
    }

    render(
      <StrictMode>
        <AssistantBubbleContentViewByContent content={content} />
        <AssistantBubbleFooter content={content} />
      </StrictMode>
    )

    expect(screen.getByText('Partial answer')).toBeInTheDocument()
    expect(screen.getByText('Network failed')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /Retry/i })).toBeInTheDocument()
  })

  it('formats one-line JSON thought details into multiline content', () => {
    const toolJson =
      '{"entries":[{"name":"ipc","is_dir":true,"size_bytes":0},{"name":"slab-server.log","is_dir":false,"size_bytes":919059921}]}'

    const { container } = render(
      <StrictMode>
        <AssistantBubbleContentViewByContent
          content={{
            ...createAssistantBubbleContent('Tool result received.'),
            item: {
              id: 'assistant-message',
              message: {
                content: 'Tool result received.',
                role: 'assistant',
                thoughts: [
                  {
                    detail: toolJson,
                    id: 'tool-call',
                    status: 'success',
                    title: 'tool_call',
                  },
                ],
              },
              status: 'success',
            },
          }}
        />
      </StrictMode>
    )

    expect(container.textContent).toContain('"entries": [')
    expect(container.textContent).toContain('"name": "slab-server.log"')
    expect(container.textContent).not.toContain('{"entries":[{"name":"ipc"')
  })
})
