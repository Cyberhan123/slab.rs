/**
 * Minimal in-test `WebSocket` stand-in for the harness JSON-RPC client, so
 * tests can drive the handshake + frames deterministically (no network, no
 * real server). Core tests inject it via the `HarnessClient` constructor's
 * `WebSocketCtor` option; ui tests stub the global via
 * `vi.stubGlobal("WebSocket", FakeWebSocket)`.
 *
 * Mode `fail` (default): fires `error` + `close` on the next tick → exercises
 * the failed-dial path. Mode `manual`: the test drives `simOpen` /
 * `simMessage` / `simClose` / `simError` on `FakeWebSocket.last`.
 */

type DOMHandler = (event: { data?: unknown; code?: number; reason?: string; message?: string; error?: { message?: string } }) => void

export class FakeWebSocket {
    /** Most recently constructed instance (the client dials one per open()). */
    static last: FakeWebSocket | undefined
    /** `fail` = auto-fail the handshake; `manual` = test drives the frames. */
    static mode: "fail" | "manual" = "fail"

    static reset(mode: "fail" | "manual" = "fail") {
        FakeWebSocket.mode = mode
        FakeWebSocket.last = undefined
    }

    readonly url: string
    readonly protocols: string | string[] | undefined
    readyState = 0 // CONNECTING
    binaryType = ""
    /** Raw frames the client sent over the socket. */
    sent: string[] = []

    private readonly handlers = new Map<string, Set<DOMHandler>>()

    constructor(url: string, protocols?: string | string[]) {
        this.url = url
        this.protocols = protocols
        FakeWebSocket.last = this
        if (FakeWebSocket.mode === "fail") {
            // Mimic a failed handshake (no server listening).
            setTimeout(() => {
                this.dispatchEvent("error", { message: "connection refused" })
                this.dispatchEvent("close", { code: 1006, reason: "" })
            }, 0)
        }
    }

    addEventListener(type: string, handler: DOMHandler): void {
        let set = this.handlers.get(type)
        if (!set) {
            set = new Set()
            this.handlers.set(type, set)
        }
        set.add(handler)
    }

    removeEventListener(type: string, handler: DOMHandler): void {
        this.handlers.get(type)?.delete(handler)
    }

    send(data: string): void {
        this.sent.push(data)
    }

    close(): void {
        this.readyState = 3
    }

    // ── test drivers ────────────────────────────────────────────────────────

    simOpen(): void {
        this.readyState = 1
        this.dispatchEvent("open", {})
    }

    simMessage(data: string): void {
        this.dispatchEvent("message", { data })
    }

    simClose(code = 1000, reason = ""): void {
        this.readyState = 3
        this.dispatchEvent("close", { code, reason })
    }

    simError(message = "websocket error"): void {
        this.dispatchEvent("error", { message })
    }

    private dispatchEvent(type: string, payload: {
        data?: unknown
        code?: number
        reason?: string
        message?: string
        error?: { message?: string }
    }): void {
        this.handlers.get(type)?.forEach((handler) => handler(payload))
    }
}
