import type { UIMessage, UIMessageChunk } from 'ai'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { SlabChatTransport } from '../chat-transport'

function userMessage(text: string): UIMessage {
  return {
    id: 'user-message',
    parts: [{ text, type: 'text' }],
    role: 'user',
  }
}

function responseFromText(text: string) {
  const stream = new ReadableStream({
    start(controller) {
      controller.enqueue(new TextEncoder().encode(text))
      controller.close()
    },
  })

  return new Response(stream, {
    status: 200,
    statusText: 'OK',
  })
}

async function collectChunks(stream: ReadableStream<UIMessageChunk>) {
  const chunks: UIMessageChunk[] = []
  const reader = stream.getReader()

  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    chunks.push(value)
  }

  return chunks
}

describe('SlabChatTransport', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('uses the configured fetch through the base transport send path', async () => {
    const globalFetch = vi.fn(() => {
      throw new Error('global fetch should not be called')
    })
    vi.stubGlobal('fetch', globalFetch)

    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        responseFromText(
          [
            'data: {"accepted":true,"action":"response_create","status":"pending","thread_id":"thread-1","type":"agent.ack"}',
            '',
            'data: {"thread_id":"thread-1","sequence_number":1,"type":"response.output_text.delta","delta":"hi"}',
            '',
            'data: {"thread_id":"thread-1","sequence_number":2,"type":"response.output_text.done","text":"hi"}',
            '',
          ].join('\n'),
        ),
      )
    const transport = new SlabChatTransport<UIMessage>({
      fetch: fetchMock,
    })

    const stream = await transport.sendMessages({
      chatId: 'chat-1',
      messages: [userMessage('hi')],
      trigger: 'submit-message',
    } as Parameters<SlabChatTransport<UIMessage>['sendMessages']>[0])

    const requestBody = JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body))
    expect(globalFetch).not.toHaveBeenCalled()
    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining('/v1/agents/responses'),
      expect.objectContaining({
        method: 'POST',
      }),
    )
    expect(requestBody).toMatchObject({
      messages: [{ content: 'hi', role: 'user' }],
      stream: true,
      type: 'agent.response.create',
    })
    expect(await collectChunks(stream)).toEqual([
      { id: 'assistant-text', type: 'text-start' },
      { delta: 'hi', id: 'assistant-text', type: 'text-delta' },
      { id: 'assistant-text', type: 'text-end' },
      { type: 'finish-step' },
      { finishReason: 'stop', type: 'finish' },
    ])
  })

  it('keeps Slab request adaptation when a caller prepares the request', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(responseFromText('data: {"type":"response.completed"}\n\n'))
    const prepareSendMessagesRequest = vi.fn(async (request) => ({
      api: 'http://localhost/custom/responses',
      body: request.body,
      credentials: 'include' as RequestCredentials,
      headers: {
        ...request.headers,
        'x-test': '1',
      },
    }))
    const transport = new SlabChatTransport<UIMessage>({
      body: {
        messages: [userMessage('body fallback')],
      },
      fetch: fetchMock,
      prepareSendMessagesRequest,
    })

    await transport.sendMessages({
      chatId: 'chat-1',
      messages: [userMessage('hello')],
      trigger: 'submit-message',
    } as Parameters<SlabChatTransport<UIMessage>['sendMessages']>[0])

    const requestInit = fetchMock.mock.calls[0]?.[1]
    const requestBody = JSON.parse(String(requestInit?.body))
    expect(prepareSendMessagesRequest).toHaveBeenCalledWith(
      expect.objectContaining({
        api: expect.stringContaining('/v1/agents/responses'),
        body: expect.objectContaining({
          stream: true,
          type: 'agent.response.create',
        }),
      }),
    )
    expect(fetchMock.mock.calls[0]?.[0]).toBe('http://localhost/custom/responses')
    expect(new Headers(requestInit?.headers).get('x-test')).toBe('1')
    expect(requestInit?.credentials).toBe('include')
    expect(requestBody).toMatchObject({
      messages: [{ content: 'hello', role: 'user' }],
      stream: true,
      type: 'agent.response.create',
    })
  })

  it('sends agent input after the stream ack supplies a thread id', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValueOnce(
        responseFromText(
          'data: {"accepted":true,"action":"response_create","status":"pending","thread_id":"thread-1","type":"agent.ack"}\n\n',
        ),
      )
      .mockResolvedValueOnce(responseFromText('data: {"type":"response.completed"}\n\n'))
    const transport = new SlabChatTransport<UIMessage>({
      fetch: fetchMock,
    })

    await collectChunks(
      await transport.sendMessages({
        chatId: 'chat-1',
        messages: [userMessage('first')],
        trigger: 'submit-message',
      } as Parameters<SlabChatTransport<UIMessage>['sendMessages']>[0]),
    )
    await collectChunks(
      await transport.sendMessages({
        chatId: 'chat-1',
        messages: [userMessage('next')],
        trigger: 'submit-message',
      } as Parameters<SlabChatTransport<UIMessage>['sendMessages']>[0]),
    )

    expect(JSON.parse(String(fetchMock.mock.calls[1]?.[1]?.body))).toMatchObject({
      content: 'next',
      stream: true,
      thread_id: 'thread-1',
      type: 'agent.input',
    })
  })

  it('starts a fresh response create when no restored thread id is supplied', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(responseFromText('data: {"type":"response.completed"}\n\n'))
    const transport = new SlabChatTransport<UIMessage>({
      fetch: fetchMock,
      model: 'model-restored',
      sessionId: 'session-restored',
    })

    await collectChunks(
      await transport.sendMessages({
        chatId: 'chat-1',
        messages: [userMessage('fresh restored session')],
        trigger: 'submit-message',
      } as Parameters<SlabChatTransport<UIMessage>['sendMessages']>[0]),
    )

    expect(JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body))).toMatchObject({
      config: {
        model: 'model-restored',
      },
      messages: [{ content: 'fresh restored session', role: 'user' }],
      session_id: 'session-restored',
      stream: true,
      type: 'agent.response.create',
    })
  })

  it('continues restored sessions when a page-level restore supplies the thread id', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(responseFromText('data: {"type":"response.completed"}\n\n'))
    const transport = new SlabChatTransport<UIMessage>({
      fetch: fetchMock,
      model: 'model-restored',
      sessionId: 'session-restored',
      threadId: 'thread-restored',
    })

    await collectChunks(
      await transport.sendMessages({
        chatId: 'chat-1',
        messages: [userMessage('continue restored')],
        trigger: 'submit-message',
      } as Parameters<SlabChatTransport<UIMessage>['sendMessages']>[0]),
    )

    expect(JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body))).toMatchObject({
      content: 'continue restored',
      stream: true,
      thread_id: 'thread-restored',
      type: 'agent.input',
    })
  })
})
