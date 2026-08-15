import { describe, expect, it } from 'vitest'

import type { AssistantUiMessage } from '../assistant-types'
import {
  extractChunkPayload,
  extractSseDeltaTextField,
  getAssistantMessageTextContent,
  getContinueGenerationPrefix,
  mergeContinuationContent,
  stripThinkTags,
  stripTrailingAssistantTurnArtifacts,
  toAssistantRequestMessage,
  toAssistantRequestMessages,
  withAssistantMessageReasoningContent,
  withAssistantMessageTextContent,
} from '../assistant-message-utils'

describe('assistant message utils', () => {
  it('reads text from string and object message content', () => {
    expect(getAssistantMessageTextContent({ content: 'hello' })).toBe('hello')
    expect(getAssistantMessageTextContent({ content: { type: 'text', text: 'from object' } })).toBe(
      'from object'
    )
    expect(getAssistantMessageTextContent({ content: { type: 'image_url' } })).toBe('')
    expect(getAssistantMessageTextContent(null)).toBe('')
  })

  it('strips trailing model control tokens and tool-call artifacts', () => {
    expect(
      stripTrailingAssistantTurnArtifacts(
        [
          'final answer',
          '',
          'tool_call id=call-1: web_search({"query":"slab"})',
          'tool_call_id: call-1',
          '',
        ].join('\n')
      )
    ).toBe('final answer')

    expect(stripTrailingAssistantTurnArtifacts('answer<|im_end|>')).toBe('answer')
    expect(stripTrailingAssistantTurnArtifacts('answer<|endoftext|>')).toBe('answer')
    expect(stripTrailingAssistantTurnArtifacts('answer<|eot_id|>')).toBe('answer')
  })

  it('drops assistant request messages that become empty after artifact stripping', () => {
    expect(
      toAssistantRequestMessage({
        role: 'assistant',
        content: 'tool_call id=call-1: web_search({"query":"slab"})',
      })
    ).toBeNull()

    expect(
      toAssistantRequestMessages([
        { role: 'user', content: 'hello' },
        { role: 'assistant', content: 'tool_call_id: call-1' },
        null,
      ])
    ).toEqual([{ role: 'user', content: 'hello' }])
  })

  it('uses the latest non-empty assistant text as continuation prefix', () => {
    expect(
      getContinueGenerationPrefix([
        { role: 'assistant', content: 'first answer' },
        { role: 'user', content: 'continue' },
      ])
    ).toBe('')

    expect(
      getContinueGenerationPrefix([
        { role: 'user', content: 'continue' },
        {
          role: 'assistant',
          content: 'partial answer\ntool_call id=call-1: search({})',
        },
      ])
    ).toBe('partial answer')
  })

  it('merges continuation output without duplicating overlap', () => {
    expect(mergeContinuationContent('', 'generated')).toBe('generated')
    expect(mergeContinuationContent('prefix', '')).toBe('prefix')
    expect(mergeContinuationContent('hello wor', 'world')).toBe('hello world')
    expect(mergeContinuationContent('hello', ' world')).toBe('hello world')
    // Full overlap: generated reproduces the prefix end-to-end.
    expect(mergeContinuationContent('hello', 'hello')).toBe('hello')
  })

  it('strips <think> reasoning blocks and leaves other content intact', () => {
    expect(stripThinkTags('<think>secret</think>visible')).toBe('visible')
    // Non-greedy across newlines.
    expect(stripThinkTags('<think>multi\nline</think>text')).toBe('text')
    // Case-insensitive tag match.
    expect(stripThinkTags('<THINK>caps</THINK>out')).toBe('out')
    // Attributes on the opening tag are tolerated.
    expect(stripThinkTags('<think role="reasoning">attrs</think>after')).toBe('after')
    // Multiple blocks in one string.
    expect(stripThinkTags('<think>a</think>x<think>b</think>y')).toBe('xy')
    // Surrounding whitespace is trimmed; an all-think string collapses to empty.
    expect(stripThinkTags('  <think>x</think>  ')).toBe('')
    // Tags that are merely prefixed by "think" (e.g. <thinking>) are left alone.
    expect(stripThinkTags('<thinking>not a think tag</thinking>')).toBe(
      '<thinking>not a think tag</thinking>'
    )
    // An unclosed tag is not removed (the regex requires a closing </think>).
    expect(stripThinkTags('before<think>never closes')).toBe('before<think>never closes')
    // Plain text without any tag is unchanged.
    expect(stripThinkTags('plain text')).toBe('plain text')
  })

  it('extracts streaming JSON payloads and delta fields safely', () => {
    const chunk = {
      data: JSON.stringify({
        choices: [
          { delta: { reasoning_content: '' } },
          { delta: { reasoning_content: 'thinking' } },
        ],
      }),
    }

    expect(extractChunkPayload(chunk)).toMatchObject({ choices: expect.any(Array) })
    expect(extractSseDeltaTextField(chunk, 'reasoning_content')).toBe('thinking')
    expect(extractSseDeltaTextField({ data: '[DONE]' }, 'content')).toBe('')
    expect(extractSseDeltaTextField({ data: '{bad json' }, 'content')).toBe('')
  })

  it('updates message text and reasoning content without losing object metadata', () => {
    const objectMessage = withAssistantMessageTextContent(
      {
        role: 'assistant',
        content: { type: 'text', text: 'old', extra: true },
      } as AssistantUiMessage,
      'new'
    )

    expect(objectMessage.content).toEqual({ type: 'text', text: 'new', extra: true })
    expect(withAssistantMessageTextContent({ role: 'assistant', content: null }, 'new').content).toBe(
      'new'
    )
    expect(
      withAssistantMessageReasoningContent({ role: 'assistant', content: 'answer' }, '  think  ')
        .reasoningContent
    ).toBe('  think  ')
    expect(
      withAssistantMessageReasoningContent({ role: 'assistant', content: 'answer' }, '   ')
        .reasoningContent
    ).toBeUndefined()
  })
})
