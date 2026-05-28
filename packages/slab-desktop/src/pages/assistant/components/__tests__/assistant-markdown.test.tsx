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
})
