/**
 * Slab chat transport for the OpenAI Responses protocol — WebSocket primary,
 * SSE fallback.
 *
 * The primary path drives the official `openai` SDK's WebSocket protocol via
 * {@link SlabResponsesWS} (a browser-friendly subclass of `ResponsesWSBase`):
 * it opens `ws://<origin>/v1/agents/responses?token=<session>` with the
 * `slab.responses` subprotocol, sends a `response.create` client event, and
 * converts the canonical `ResponsesServerEvent` frames into AI-SDK
 * `UIMessageChunk`s. The base owns the send-queue + dispatch; we just pump
 * `.on("event", …)`.
 *
 * Degradation: if the WS cannot establish (connect error, abnormal close, or
 * open timeout) BEFORE any chunk is written, we fall back to the SDK's HTTP/SSE
 * path (`client.responses.create({ stream: true })`). Once a chunk has been
 * written over WS we commit to it — a mid-stream failure surfaces as an error
 * chunk rather than duplicating the stream. Multi-turn is carried by
 * `previous_response_id` (the slab thread id returned in `response.completed`).
 */

import OpenAI from "openai"

import {
    type ChatTransport,
    type UIMessage,
    type UIMessageChunk,
    createUIMessageStream,
} from "ai"

import { SERVER_BASE_URL } from "@slab/api/config"

import { SlabResponsesWS } from "./openai-responses/responses-ws"
import { convertEvent, createStreamState } from "./openai-responses/stream"
import type { ResponseStreamEvent } from "./openai-responses/types"

export interface SlabChatTransportOptions {
    /** Model id sent as `model` (defaults to "slab-llama"). */
    model?: string
    /** Slab session id, sent as the SDK `apiKey` → `Authorization: Bearer <sessionId>` (SSE) / `?token=<sessionId>` (WS). */
    sessionId?: string
    /** Initial thread/response id (from a restored session) for `previous_response_id` chaining. */
    threadId?: string | null
    /** Override the SDK base URL (default `${SERVER_BASE_URL}/v1/agents`; the SDK appends `/responses`). */
    baseURL?: string
    /**
     * Inject a pre-built `openai` client (tests pass a mock). When omitted a
     * client is constructed with `dangerouslyAllowBrowser: true` (the "apiKey"
     * is the local slab session id, not a secret OpenAI key).
     */
    client?: OpenAI
}

const DEFAULT_MODEL = "slab-llama"
/** Cap on how long we wait for the WS handshake before degrading to SSE. */
const WS_OPEN_TIMEOUT_MS = 5000

/** Extract the latest user message text to send as the new turn's `input`. */
function lastUserInput(messages: UIMessage[]): string {
    for (let i = messages.length - 1; i >= 0; i -= 1) {
        const message = messages[i]
        if (message.role !== "user") continue
        return message.parts
            .filter((part): part is Extract<(typeof message.parts)[number], { type: "text" }> => part.type === "text")
            .map((part) => part.text)
            .join("")
            .trim()
    }
    return ""
}

/** `response.create` client event body shared by the WS and SSE turns. */
interface CreateResponseInput {
    model: string
    input: string
    previousResponseId: string | null
}

export class SlabChatTransport<UI_MESSAGE extends UIMessage> implements ChatTransport<UI_MESSAGE> {
    private readonly client: OpenAI
    private readonly model: string
    private threadId: string | null

    constructor(options: SlabChatTransportOptions = {}) {
        this.model = options.model ?? DEFAULT_MODEL
        this.threadId = options.threadId ?? null
        this.client =
            options.client ??
            new OpenAI({
                baseURL: options.baseURL ?? `${SERVER_BASE_URL}/v1/agents`,
                apiKey: options.sessionId ?? "slab",
                // slab-desktop runs in the Tauri webview; the "apiKey" is the
                // local slab session id, not a secret OpenAI key.
                dangerouslyAllowBrowser: true,
            })
    }

    async sendMessages(options: {
        messages: UI_MESSAGE[]
        abortSignal?: AbortSignal
    }): Promise<ReadableStream<UIMessageChunk>> {
        const input = lastUserInput(options.messages)
        const turn: CreateResponseInput = {
            model: this.model,
            input,
            previousResponseId: this.threadId,
        }

        return createUIMessageStream({
            execute: async ({ writer }) => {
                // Try the WebSocket transport first; fall back to SSE if it can't
                // establish before producing any output.
                const ws = await this.openResponsesWS(options.abortSignal)
                if (ws) {
                    await this.runWsTurn(ws, turn, writer, options.abortSignal)
                    return
                }
                await this.runSseTurn(turn, writer, options.abortSignal)
            },
            onError: (error) => (error instanceof Error ? error.message : "stream error"),
        })
    }

    async reconnectToStream(_options: {
        chatId: string
    }): Promise<ReadableStream<UIMessageChunk> | null> {
        // No server-side resumable stream today (the GET SSE resume path is a
        // future enhancement). `null` tells `useChat` there is nothing to reconnect.
        return null
    }

    // ── WebSocket turn ──────────────────────────────────────────────────────

    /** Construct + open the WS; returns `null` if it fails to establish. */
    private async openResponsesWS(signal?: AbortSignal): Promise<SlabResponsesWS | null> {
        try {
            const ws = new SlabResponsesWS(this.client)
            // The base re-emits socket errors on its emitter; bind a no-op sink so
            // an establish-time error never surfaces as an unhandled emitter
            // `error` (Node throws if `error` is emitted with no listener).
            // `runWsTurn` adds the real handler once the turn starts.
            ws.on("error", () => {})
            await this.awaitWsOpen(ws, signal)
            return ws
        } catch (error) {
            // Any establishment failure (construction, handshake, open timeout)
            // degrades to SSE — never crash the chat over a transport choice.
            console.debug("[slab-chat] WS establish failed, falling back to SSE:", error)
            return null
        }
    }

    /** Resolves on `'open'`; rejects on `'error'`, close-before-open, or timeout. */
    private awaitWsOpen(ws: SlabResponsesWS, signal?: AbortSignal): Promise<void> {
        const socket = ws.socket
        return new Promise<void>((resolve, reject) => {
            if (socket.readyState === 1) {
                resolve()
                return
            }
            const cleanup = () => {
                clearTimeout(timeout)
                socket.off("open", onOpen)
                socket.off("error", onError)
                socket.off("close", onClose)
                signal?.removeEventListener("abort", onAbort)
            }
            const timeout = setTimeout(() => {
                cleanup()
                reject(new Error("websocket open timed out"))
            }, WS_OPEN_TIMEOUT_MS)
            const onOpen = () => {
                cleanup()
                resolve()
            }
            const onError = (err: unknown) => {
                cleanup()
                reject(err instanceof Error ? err : new Error("websocket error"))
            }
            const onClose = () => {
                cleanup()
                reject(new Error("websocket closed before opening"))
            }
            const onAbort = () => {
                cleanup()
                reject(new Error("aborted"))
            }
            socket.on("open", onOpen)
            socket.on("error", onError)
            socket.on("close", onClose)
            signal?.addEventListener("abort", onAbort, { once: true })
        })
    }

    /**
     * Pump canonical events from the open WS into the AI-SDK stream writer.
     * Only called after `awaitWsOpen` resolved, so a failure here is mid-stream
     * (surfaces as an error chunk — no SSE fallback, which would duplicate).
     */
    private runWsTurn(
        ws: SlabResponsesWS,
        turn: CreateResponseInput,
        writer: { write: (chunk: UIMessageChunk) => void },
        signal?: AbortSignal,
    ): Promise<void> {
        return new Promise<void>((resolve) => {
            const state = createStreamState()
            const cleanup = () => {
                ws.off("event", onEvent as (event: ResponseStreamEvent) => void)
                ws.off("error", onError)
                ws.off("close", onClose)
                signal?.removeEventListener("abort", onAbort)
                try {
                    ws.close()
                } catch {
                    // ignore
                }
            }
            const finish = () => {
                cleanup()
                resolve()
            }
            const onAbort = () => {
                finish()
            }
            const onEvent = (event: ResponseStreamEvent) => {
                if (event.type === "response.completed") {
                    this.threadId = event.response.id
                }
                for (const chunk of convertEvent(event, state)) {
                    writer.write(chunk)
                }
                if (
                    event.type === "response.completed" ||
                    event.type === "response.failed" ||
                    event.type === "response.incomplete"
                ) {
                    finish()
                }
            }
            const onError = (err: unknown) => {
                const message = err instanceof Error ? err.message : "websocket error"
                writer.write({ errorText: message, type: "error" })
                writer.write({ finishReason: "error", type: "finish" })
                finish()
            }
            const onClose = () => {
                // Graceful close without a terminal event — finish the stream so
                // the UI doesn't hang waiting for `response.completed`.
                if (!state.finished) {
                    writer.write({ finishReason: "stop", type: "finish" })
                }
                finish()
            }
            ws.on("event", onEvent as (event: ResponseStreamEvent) => void)
            ws.on("error", onError)
            ws.on("close", onClose)
            signal?.addEventListener("abort", onAbort, { once: true })
            ws.send({
                type: "response.create",
                model: turn.model,
                input: turn.input,
                stream: true,
                ...(turn.previousResponseId ? { previous_response_id: turn.previousResponseId } : {}),
            })
        })
    }

    // ── SSE fallback turn (the Phase G SDK-client path) ──────────────────────

    private async runSseTurn(
        turn: CreateResponseInput,
        writer: { write: (chunk: UIMessageChunk) => void },
        signal?: AbortSignal,
    ): Promise<void> {
        let stream: AsyncIterable<ResponseStreamEvent>
        try {
            stream = await this.client.responses.create(
                {
                    model: turn.model,
                    input: turn.input,
                    stream: true,
                    ...(turn.previousResponseId
                        ? { previous_response_id: turn.previousResponseId }
                        : {}),
                },
                { signal },
            )
        } catch (error) {
            const message = error instanceof Error ? error.message : "request failed"
            writer.write({ errorText: message, type: "error" })
            writer.write({ finishReason: "error", type: "finish" })
            return
        }

        const state = createStreamState()
        for await (const event of stream) {
            if (event.type === "response.completed") {
                this.threadId = event.response.id
            }
            for (const chunk of convertEvent(event, state)) {
                writer.write(chunk)
            }
        }
    }
}
