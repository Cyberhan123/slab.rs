import { afterEach, describe, expect, it, vi } from 'vitest'

import { createWebSocketFetch } from '../websocket-client'

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

class FakeWebSocket extends EventTarget {
  static readonly OPEN = 1

  readonly sent: string[] = []
  readyState = FakeWebSocket.OPEN

  constructor(readonly url: string) {
    super()
    queueMicrotask(() => this.dispatchEvent(new Event('open')))
  }

  close() {
    this.dispatchEvent(new Event('close'))
  }

  emit(data: string) {
    this.dispatchEvent(new MessageEvent('message', { data }))
  }

  send(data: string) {
    this.sent.push(data)
  }
}

class FailingWebSocket extends EventTarget {
  static readonly OPEN = 1

  readonly readyState = 0

  constructor(readonly url: string) {
    super()
    queueMicrotask(() => this.dispatchEvent(new Event('error')))
  }

  close() {}

  send() {}
}

describe('createWebSocketFetch', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('uses websocket for streamed response posts and exposes an SSE-compatible response', async () => {
    const sockets: FakeWebSocket[] = []
    vi.stubGlobal(
      'WebSocket',
      class extends FakeWebSocket {
        constructor(url: string) {
          super(url)
          sockets.push(this)
        }
      },
    )
    const fetch = createWebSocketFetch()

    const response = await fetch('http://localhost:3000/v1/agents/responses', {
      body: JSON.stringify({
        input: [],
        stream: true,
      }),
      method: 'POST',
    })
    const textPromise = response.text()

    expect(sockets[0]?.url).toBe('ws://localhost:3000/v1/agents/responses')
    expect(JSON.parse(String(sockets[0]?.sent[0]))).toEqual({
      input: [],
      type: 'response.create',
    })

    sockets[0]?.emit('{"thread_id":"thread-1","sequence_number":1,"type":"response.output_text.delta","delta":"hi"}')
    sockets[0]?.emit('{"thread_id":"thread-1","sequence_number":2,"type":"response.completed"}')

    expect(await textPromise).toBe(
      [
        'data: {"thread_id":"thread-1","sequence_number":1,"type":"response.output_text.delta","delta":"hi"}',
        '',
        'data: {"thread_id":"thread-1","sequence_number":2,"type":"response.completed"}',
        '',
        'data: [DONE]',
        '',
        '',
      ].join('\n'),
    )
  })

  it('falls back to canonical post plus SSE stream when websocket connection fails', async () => {
    vi.stubGlobal('WebSocket', FailingWebSocket)
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            id: 'thread-1',
            object: 'response',
            status: 'in_progress',
          }),
        ),
      )
      .mockResolvedValueOnce(
        responseFromText(
          'data: {"thread_id":"thread-1","sequence_number":1,"type":"response.output_text.delta","delta":"hi"}\n\n',
        ),
      )
    vi.stubGlobal('fetch', fetchMock)
    const fetch = createWebSocketFetch({ url: 'ws://localhost:3000/v1/agents/responses' })

    const response = await fetch('http://localhost:3000/v1/agents/responses', {
      body: JSON.stringify({
        input: [],
        stream: true,
      }),
      method: 'POST',
    })

    expect(JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body))).toEqual({
      input: [],
      stream: false,
    })
    expect(fetchMock.mock.calls[1]?.[0]).toBe(
      'http://localhost:3000/v1/agents/responses?transport=sse&thread_id=thread-1',
    )
    expect(await response.text()).toBe(
      [
        'data: {"id":"thread-1","object":"response","status":"in_progress"}',
        '',
        'data: {"thread_id":"thread-1","sequence_number":1,"type":"response.output_text.delta","delta":"hi"}',
        '',
        '',
      ].join('\n'),
    )
  })

  it('delegates non-streamed requests to global fetch', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(new Response('ok'))
    vi.stubGlobal('fetch', fetchMock)
    const fetch = createWebSocketFetch({ url: 'ws://localhost:3000/v1/agents/responses' })

    const response = await fetch('http://localhost:3000/v1/agents/responses', {
      body: JSON.stringify({
        input: '',
        stream: false,
      }),
      method: 'POST',
    })

    expect(await response.text()).toBe('ok')
    expect(fetchMock).toHaveBeenCalledOnce()
  })
})
