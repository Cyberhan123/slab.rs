import OpenAI from 'openai'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { FakeWebSocket } from '../../__tests__/fake-websocket'
import { SlabResponsesWS } from '../responses-ws'
import type { ResponseStreamEvent } from '../types'

const tick = () => new Promise<void>((resolve) => setTimeout(resolve, 0))

function newClient(apiKey = 'sess-123'): OpenAI {
    return new OpenAI({
        apiKey,
        baseURL: 'http://localhost:3000/v1/agents',
        dangerouslyAllowBrowser: true,
    })
}

beforeEach(() => {
    FakeWebSocket.reset('manual')
    vi.stubGlobal('WebSocket', FakeWebSocket)
})

afterEach(() => {
    vi.unstubAllGlobals()
})

describe('SlabResponsesWS (browser subclass of ResponsesWSBase)', () => {
    it('connects to the slab WS path with ?token= and the slab.responses subprotocol', () => {
        const ws = new SlabResponsesWS(newClient('sess-123'))
        const fake = FakeWebSocket.last
        expect(fake).toBeDefined()
        expect(fake!.protocols).toBe('slab.responses')
        expect(fake!.url).toContain('ws://localhost:3000/v1/agents/responses')
        expect(fake!.url).toContain('token=sess-123')
        ws.close()
    })

    it('send() serializes a response.create client event to the underlying socket', async () => {
        const ws = new SlabResponsesWS(newClient())
        const fake = FakeWebSocket.last!
        fake.simOpen()
        await tick()
        ws.send({
            type: 'response.create',
            model: 'slab-llama',
            input: 'hi',
            stream: true,
        } as never)
        expect(fake.sent).toHaveLength(1)
        expect(fake.sent[0]).toMatch(/"type":"response.create"/)
        expect(fake.sent[0]).toMatch(/"input":"hi"/)
        ws.close()
    })

    it('dispatches incoming frames as parsed `event` emissions', async () => {
        const ws = new SlabResponsesWS(newClient())
        const fake = FakeWebSocket.last!
        fake.simOpen()
        await tick()

        const received: ResponseStreamEvent[] = []
        ws.on('event', (event) => {
            received.push(event as ResponseStreamEvent)
        })

        fake.simMessage(
            JSON.stringify({
                type: 'response.created',
                response: {
                    id: 'resp_1',
                    object: 'response',
                    created_at: 0,
                    status: 'in_progress',
                    output: [],
                },
            }),
        )
        await tick()

        expect(received[0]).toMatchObject({ type: 'response.created' })
        ws.close()
    })
})
