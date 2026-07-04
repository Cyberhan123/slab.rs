import {
    HttpChatTransport,
    type UIMessage,
    type UIMessageChunk,
    type HttpChatTransportInitOptions,
} from 'ai';

import { SERVER_BASE_URL } from '@slab/api/config'

import { SlabAgentChatAdapter } from './slab-agent-chat-adapter'

function isTerminalChunk(chunk: UIMessageChunk) {
    return chunk.type === "finish" || chunk.type === "error" || chunk.type === "abort"
}

export class SlabChatTransport<
    UI_MESSAGE extends UIMessage,
> extends HttpChatTransport<UI_MESSAGE> {
    private readonly adapter = new SlabAgentChatAdapter()

    constructor(options: HttpChatTransportInitOptions<UI_MESSAGE> = {}) {
        super({
            api: `${SERVER_BASE_URL}/v1/agents/responses`,
            ...options,
        });
    }

    async sendMessages(
        options: Parameters<HttpChatTransport<UI_MESSAGE>['sendMessages']>[0],
    ): Promise<ReadableStream<UIMessageChunk>> {
        const response = await fetch(this.api, {
            body: JSON.stringify(this.adapter.createCommand(options)),
            headers: {
                "Content-Type": "application/json",
            },
            method: "POST",
            signal: options.abortSignal,
        })

        if (!response.ok) {
            throw new Error((await response.text()) || "Failed to fetch the chat response.")
        }

        const serverMessage = await response.json()
        const initialChunks = this.adapter.handleServerMessage(serverMessage)
        const streamUrl = serverMessage.thread_id
            ? `${this.api}?transport=sse&thread_id=${encodeURIComponent(serverMessage.thread_id)}`
            : null

        if (!streamUrl) {
            return new ReadableStream<UIMessageChunk>({
                start: (controller) => {
                    for (const chunk of initialChunks) {
                        controller.enqueue(chunk)
                    }
                    controller.close()
                },
            })
        }

        const streamResponse = await fetch(streamUrl, {
            headers: {
                Accept: "text/event-stream",
            },
            signal: options.abortSignal,
        })

        if (!streamResponse.ok) {
            throw new Error((await streamResponse.text()) || "Failed to fetch the chat stream.")
        }

        if (!streamResponse.body) {
            throw new Error("The response body is empty.")
        }

        return this.processResponseStream(streamResponse.body, initialChunks)
    }

    protected processResponseStream(
        stream: ReadableStream<Uint8Array<ArrayBufferLike>>,
        initialChunks: UIMessageChunk[] = [],
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
                start(controller) {
                    for (const chunk of initialChunks) {
                        controller.enqueue(chunk)
                    }
                },
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
