import type { UIMessage, UIMessageChunk } from 'ai'
import OpenAI from 'openai'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { SlabChatTransport } from '../chat-transport'
import type { ResponseOutputItem, ResponseStreamEvent } from '../openai-responses/types'
import { FakeWebSocket } from './fake-websocket'

/** Build an async iterable of canonical events (stands in for the SDK `Stream`). */
function eventStream(events: ResponseStreamEvent[]): AsyncIterable<ResponseStreamEvent> {
  return {
    async *[Symbol.asyncIterator]() {
      for (const event of events) {
        yield event
      }
    },
  }
}

function userMessage(text: string): UIMessage {
  return { id: 'user-1', parts: [{ text, type: 'text' }], role: 'user' }
}

const messageItem = { type: 'message', role: 'assistant', content: [] } as unknown as ResponseOutputItem
const finalizedMessage = {
  type: 'message',
  role: 'assistant',
  content: [{ type: 'output_text', text: 'hi', annotations: [] }],
} as unknown as ResponseOutputItem

function asEvent(e: unknown): ResponseStreamEvent {
  return e as ResponseStreamEvent
}

/** A canonical text-stream sequence ending in response.completed with thread id `tid`. */
function textRunEvents(tid: string): ResponseStreamEvent[] {
  return [
    asEvent({ type: 'response.output_item.added', output_index: 0, item: messageItem, sequence_number: 1 }),
    asEvent({ type: 'response.output_text.delta', output_index: 0, content_index: 0, delta: 'hi', item_id: 'i', sequence_number: 2 }),
    asEvent({ type: 'response.output_item.done', output_index: 0, item: finalizedMessage, sequence_number: 3 }),
    asEvent({
      type: 'response.completed',
      sequence_number: 4,
      response: { id: tid, object: 'response', created_at: 0, status: 'completed', output: [] },
    }),
  ]
}

/** A real `OpenAI` client so `ResponsesWSBase` can derive the WS URL etc.; only
 * `responses.create` is spied (so the SSE path is deterministic + network-free). */
function realClient(): OpenAI {
  return new OpenAI({
    apiKey: 'sess',
    baseURL: 'http://localhost:3000/v1/agents',
    dangerouslyAllowBrowser: true,
  })
}

function mockClient(events: ResponseStreamEvent[]) {
  const client = realClient()
  const create = vi.spyOn(client.responses, 'create')
  create.mockResolvedValue(eventStream(events) as never)
  return { client, create }
}

function mockRejectingClient(message = 'network down') {
  const client = realClient()
  const create = vi.spyOn(client.responses, 'create')
  create.mockRejectedValue(new Error(message))
  return { client, create }
}

async function collectChunks(stream: ReadableStream<UIMessageChunk>): Promise<UIMessageChunk[]> {
  const chunks: UIMessageChunk[] = []
  const reader = stream.getReader()
  while (true) {
    // eslint-disable-next-line no-await-in-loop -- sequential stream reads
    const { done, value } = await reader.read()
    if (done) break
    chunks.push(value)
  }
  return chunks
}

const tick = () => new Promise<void>((resolve) => setTimeout(resolve, 0))

const sendArgs = (transport: SlabChatTransport<UIMessage>, messages: UIMessage[]) =>
  transport.sendMessages({ chatId: 'c', messages, trigger: 'submit-message' } as Parameters<
    SlabChatTransport<UIMessage>['sendMessages']
  >[0])

beforeEach(() => {
  // Default: the WS handshake auto-fails, so the transport degrades to SSE.
  FakeWebSocket.reset('fail')
  vi.stubGlobal('WebSocket', FakeWebSocket)
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('SlabChatTransport — SSE fallback (WS handshake fails)', () => {
  it('falls back to POST /v1/agents/responses and streams canonical events into UI chunks', async () => {
    const { client, create } = mockClient(textRunEvents('thread-1'))
    const transport = new SlabChatTransport<UIMessage>({ client, model: 'slab-llama' })

    const stream = await sendArgs(transport, [userMessage('hi')])
    const chunks = await collectChunks(stream)

    // WS was attempted (a socket was constructed) but the SDK SSE path was used.
    expect(FakeWebSocket.last).toBeDefined()
    expect(create).toHaveBeenCalledTimes(1)
    expect(create.mock.calls[0]?.[0]).toMatchObject({ model: 'slab-llama', input: 'hi', stream: true })
    expect(create.mock.calls[0]?.[0]).not.toHaveProperty('previous_response_id')

    expect(chunks).toEqual([
      { id: 'assistant-text', type: 'text-start' },
      { delta: 'hi', id: 'assistant-text', type: 'text-delta' },
      { id: 'assistant-text', type: 'text-end' },
      { type: 'finish-step' },
      { finishReason: 'stop', type: 'finish' },
    ])
  })

  it('chains the next turn on previous_response_id from response.completed (SSE)', async () => {
    const { client, create } = mockClient(textRunEvents('thread-9'))
    const transport = new SlabChatTransport<UIMessage>({ client })

    await collectChunks(await sendArgs(transport, [userMessage('first')]))
    expect(create.mock.calls[0]?.[0]).not.toHaveProperty('previous_response_id')

    await collectChunks(await sendArgs(transport, [userMessage('second')]))
    expect(create.mock.calls[1]?.[0]).toMatchObject({ input: 'second', previous_response_id: 'thread-9' })
  })

  it('starts with previous_response_id when constructed with a restored threadId (SSE)', async () => {
    const { client, create } = mockClient(textRunEvents('thread-restored'))
    const transport = new SlabChatTransport<UIMessage>({ client, threadId: 'thread-restored' })

    await collectChunks(await sendArgs(transport, [userMessage('continue')]))

    expect(create.mock.calls[0]?.[0]).toMatchObject({
      input: 'continue',
      previous_response_id: 'thread-restored',
    })
  })

  it('emits an error + finish when the SSE create() rejects', async () => {
    const { client, create } = mockRejectingClient('network down')
    const transport = new SlabChatTransport<UIMessage>({ client })

    const chunks = await collectChunks(await sendArgs(transport, [userMessage('hi')]))

    expect(create).toHaveBeenCalledTimes(1)
    expect(chunks[0]).toMatchObject({ type: 'error', errorText: 'network down' })
    expect(chunks.at(-1)).toMatchObject({ type: 'finish', finishReason: 'error' })
  })

  it('reconnectToStream returns null (no server resumable stream)', async () => {
    const { client } = mockClient([])
    const transport = new SlabChatTransport<UIMessage>({ client })
    expect(await transport.reconnectToStream({ chatId: 'c' })).toBeNull()
  })
})

describe('SlabChatTransport — WebSocket primary', () => {
  beforeEach(() => FakeWebSocket.reset('manual'))

  /** Drive the captured fake socket through a full canonical turn. */
  async function driveWsTurn(events: ResponseStreamEvent[]): Promise<void> {
    // Let execute() construct the WS (it runs when the stream is first read).
    await tick()
    const fake = FakeWebSocket.last
    if (!fake) throw new Error('FakeWebSocket was not constructed')
    fake.simOpen()
    // awaitWsOpen resolves → runWsTurn registers listeners + sends response.create:
    await tick()
    for (const event of events) {
      fake.simMessage(JSON.stringify(event))
      // eslint-disable-next-line no-await-in-loop -- sequential frame simulation
      await tick()
    }
  }

  it('uses the WS transport when it establishes and does NOT call responses.create', async () => {
    const { client, create } = mockClient(textRunEvents('thread-ws'))
    const transport = new SlabChatTransport<UIMessage>({ client, model: 'slab-llama' })

    const stream = await sendArgs(transport, [userMessage('hi')])
    const chunksPromise = collectChunks(stream)

    await driveWsTurn(textRunEvents('thread-ws'))
    const chunks = await chunksPromise

    // WS path taken — SSE never touched.
    expect(create).not.toHaveBeenCalled()
    // The transport sent a canonical `response.create` client event.
    expect(FakeWebSocket.last?.sent[0]).toMatch(/"type":"response.create"/)
    expect(FakeWebSocket.last?.sent[0]).toMatch(/"input":"hi"/)

    expect(chunks).toEqual([
      { id: 'assistant-text', type: 'text-start' },
      { delta: 'hi', id: 'assistant-text', type: 'text-delta' },
      { id: 'assistant-text', type: 'text-end' },
      { type: 'finish-step' },
      { finishReason: 'stop', type: 'finish' },
    ])
  })

  it('chains the next turn on the response.completed thread id (WS)', async () => {
    const { client, create } = mockClient(textRunEvents('never'))
    const transport = new SlabChatTransport<UIMessage>({ client })

    // First turn (WS) → captures thread-42 from response.completed.
    const chunks1 = collectChunks(await sendArgs(transport, [userMessage('one')]))
    await driveWsTurn(textRunEvents('thread-42'))
    await chunks1

    // Second turn (WS) → carries previous_response_id = thread-42.
    const chunks2 = collectChunks(await sendArgs(transport, [userMessage('two')]))
    await driveWsTurn(textRunEvents('thread-42'))
    await chunks2

    expect(create).not.toHaveBeenCalled()
    expect(FakeWebSocket.last?.sent[0]).toMatch(/"previous_response_id":"thread-42"/)
    expect(FakeWebSocket.last?.sent[0]).toMatch(/"input":"two"/)
  })
})
