import { Bubble, ThoughtChain, XProvider, type BubbleListProps } from '@ant-design/x'
import { render, screen } from '@testing-library/react'
import { StrictMode, useEffect, useState } from 'react'
import { describe, expect, it } from 'vitest'

import { AssistantMarkdown } from '../assistant-markdown'

function StreamingThought() {
  const [content, setContent] = useState('查询中')

  useEffect(() => {
    setContent('查询中\n\n正在调用工具')
  }, [])

  const items = [
    {
      blink: true,
      collapsible: true,
      content: (
        <AssistantMarkdown className="assistant-markdown--assistant" hasNextChunk>
          {content}
        </AssistantMarkdown>
      ),
      key: 'thinking',
      status: 'loading' as const,
      title: 'Thinking',
    },
  ]

  return (
    <ThoughtChain
      items={items}
      defaultExpandedKeys={items.map((item) => item.key)}
    />
  )
}

describe('AssistantMarkdown', () => {
  it('renders latex, citations, and fenced code', async () => {
    render(
      <AssistantMarkdown>
        {'Inline $x^2$ citation<sup><a href="https://example.com">1</a></sup>\n\n```diff\n+ added\n```'}
      </AssistantMarkdown>
    )

    expect(screen.getByText('1')).toBeInTheDocument()
    expect(screen.getByText('+ added')).toBeInTheDocument()
  })

  it('renders inside Bubble.List without recursive updates', () => {
    const roles = {
      assistant: {
        contentRender: (content: string) => (
          <AssistantMarkdown className="assistant-markdown--assistant">
            {content}
          </AssistantMarkdown>
        ),
        placement: 'start',
        variant: 'filled',
      },
    } satisfies BubbleListProps['role']

    render(
      <StrictMode>
        <XProvider>
          <Bubble.List
            role={roles}
            items={[
              {
                content: '帮我查询一下日本今天天气',
                key: 'assistant-message',
                role: 'assistant',
              },
            ]}
          />
        </XProvider>
      </StrictMode>
    )

    expect(screen.getByText('帮我查询一下日本今天天气')).toBeInTheDocument()
  })

  it('renders streaming thought content without recursive updates', () => {
    render(
      <StrictMode>
        <XProvider>
          <StreamingThought />
        </XProvider>
      </StrictMode>
    )

    expect(screen.getByText('查询中')).toBeInTheDocument()
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
        <XProvider>
          <Bubble.List
            role={{
              assistant: {
                contentRender: (value: string) => (
                  <AssistantMarkdown className="assistant-markdown--assistant">
                    {value}
                  </AssistantMarkdown>
                ),
                placement: 'start',
                variant: 'filled',
              },
            }}
            items={[
              {
                content,
                key: 'assistant-message',
                role: 'assistant',
              },
            ]}
          />
        </XProvider>
      </StrictMode>
    )

    expect(screen.getByText('I can help with the following tasks:')).toBeInTheDocument()
    expect(screen.queryByText('The assistant planned the answer.')).not.toBeInTheDocument()
    expect(
      screen.getByText(/This paragraph intentionally looks like unrelated retrieved text/)
    ).toBeInTheDocument()
  })
})
