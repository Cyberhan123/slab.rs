import {
    HttpChatTransport,
    type UIMessage,
    type UIMessageChunk,
    type HttpChatTransportInitOptions,
} from 'ai';

import { SERVER_BASE_URL } from '@slab/api/config'

import { agentResponsesWebSocketUrl } from './assistant-agent-state'
import { SlabAgentChatAdapter } from './slab-agent-chat-adapter'
import { createWebSocketFetch } from './websocket-client'

export type SlabChatTransportOptions<
    UI_MESSAGE extends UIMessage,
> = HttpChatTransportInitOptions<UI_MESSAGE> & {
    chunkDelayMs?: number
    model?: string
    sessionId?: string
    threadId?: string | null
}

function isTerminalChunk(chunk: UIMessageChunk) {
    return chunk.type === "finish" || chunk.type === "error" || chunk.type === "abort"
}

export class SlabChatTransport<
    UI_MESSAGE extends UIMessage,
> extends HttpChatTransport<UI_MESSAGE> {
    private readonly adapter: SlabAgentChatAdapter

    constructor(options: SlabChatTransportOptions<UI_MESSAGE> = {}) {
        const { model, sessionId, threadId, ...transportOptions } = options
        const adapter = new SlabAgentChatAdapter({ model, sessionId, threadId })
        const prepareSendMessagesRequest = transportOptions.prepareSendMessagesRequest

        super({
            ...transportOptions,
            api: transportOptions.api ?? `${SERVER_BASE_URL}/v1/agents/responses`,
            fetch: transportOptions.fetch ?? createWebSocketFetch({ url: agentResponsesWebSocketUrl() }),
            prepareSendMessagesRequest: async (request) => {
                const body = {
                    ...adapter.createCommand({
                        body: request.body,
                        messages: request.messages,
                    }),
                    stream: true,
                }

                if (!prepareSendMessagesRequest) {
                    return {
                        body,
                    }
                }

                const preparedRequest = await prepareSendMessagesRequest({
                    ...request,
                    body,
                })

                return {
                    ...preparedRequest,
                    body: preparedRequest?.body ?? body,
                }
            },
        });

        this.adapter = adapter
    }

    protected processResponseStream(
        stream: ReadableStream<Uint8Array<ArrayBufferLike>>,
    ): ReadableStream<UIMessageChunk> {
        const adapter = this.adapter
        const decoder = new TextDecoder()
        let buffer = ""

        function enqueuePayload(
            data: string,
            controller: TransformStreamDefaultController<UIMessageChunk>,
        ) {
            let terminal = false

            for (const uiChunk of adapter.transformPayload(data)) {
                controller.enqueue(uiChunk)
                terminal ||= isTerminalChunk(uiChunk)
            }

            if (terminal) {
                controller.terminate()
            }
        }

        return stream.pipeThrough(
            new TransformStream<Uint8Array<ArrayBufferLike>, UIMessageChunk>({
                transform(chunk, controller) {
                    buffer += decoder.decode(chunk, { stream: true })
                    const events = buffer.split(/\r?\n\r?\n/)
                    buffer = events.pop() ?? ""

                    for (const event of events) {
                        const data = event
                            .split(/\r?\n/)
                            .filter((line) => line.startsWith("data:"))
                            .map((line) => line.slice("data:".length).trimStart())
                            .join("\n")

                        if (!data || data === "[DONE]") {
                            continue
                        }

                        enqueuePayload(data, controller)
                    }
                },
                flush(controller) {
                    buffer += decoder.decode()
                    const data = buffer
                        .split(/\r?\n/)
                        .filter((line) => line.startsWith("data:"))
                        .map((line) => line.slice("data:".length).trimStart())
                        .join("\n")

                    if (data && data !== "[DONE]") {
                        enqueuePayload(data, controller)
                    }
                },
            }),
        )
    }
}
