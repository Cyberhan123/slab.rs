import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { AssistantMarkdown } from '../assistant-markdown'

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
})
