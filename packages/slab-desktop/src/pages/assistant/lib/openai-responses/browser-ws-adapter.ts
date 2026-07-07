/**
 * Browser WebSocket adapter for the openai SDK's `ResponsesWSBase`.
 *
 * Vendored from the SDK's internal `openai/internal/ws-adapter-browser` — that
 * path is NOT exported by the package (no `./internal/*` entry in `exports`),
 * so we carry the (small, self-contained) adapter here. It wraps the native
 * browser `WebSocket` to satisfy the `WebSocketLike` interface that
 * `ResponsesWSBase<TSocket>` requires, translating DOM `addEventListener`
 * events into the Node-style positional `.on('message'/'open'/'close'/'error')`
 * callbacks the base expects.
 */

/* eslint-disable @typescript-eslint/no-explicit-any, no-underscore-dangle -- The
 * listener type is `(...args: any[]) => void` to match the SDK's `WebSocketLike`
 * exactly (bivalence is what lets this single-signature class satisfy the SDK's
 * overloaded `on('error', (err: Error) => void)` and the
 * `ResponsesWSBase<BrowserWebSocket>` constraint), and the `_`-prefixed members
 * mirror the SDK's internal adapter verbatim. Faithful vendored port. */

/** Normalized socket interface abstracting over Node `ws` and the browser WS. */
export interface WebSocketLike {
    readonly readyState: number;
    send(data: string | ArrayBufferLike | ArrayBufferView): void;
    close(code?: number, reason?: string): void;
    on(event: "open", listener: () => void): void;
    on(
        event: "message",
        listener: (data: string | ArrayBuffer | ArrayBufferView, isBinary: boolean) => void,
    ): void;
    on(event: "close", listener: (code: number, reason: string) => void): void;
    on(event: "error", listener: (err: Error) => void): void;
    on(event: string, listener: (...args: any[]) => void): void;
    off(event: string, listener: (...args: any[]) => void): void;
    once(event: string, listener: (...args: any[]) => void): void;
}

/** Standard WebSocket readyState values (RFC 6455). */
export const ReadyState = {
    CONNECTING: 0,
    OPEN: 1,
    CLOSING: 2,
    CLOSED: 3,
} as const;

type Listener = (...args: any[]) => void;
type DOMEventHandler = (event: Event) => void;

/**
 * Adapts a native browser `WebSocket` to `WebSocketLike`. Faithful port of the
 * SDK's internal adapter — the DOM→positional conversion lives in
 * {@link BrowserWebSocket.prototype._wrapListener}.
 */
export class BrowserWebSocket implements WebSocketLike {
    private readonly _ws: WebSocket;
    private readonly _listenerMap = new Map<string, Map<Listener, DOMEventHandler>>();

    constructor(ws: WebSocket) {
        this._ws = ws;
        this._ws.binaryType = "arraybuffer";
    }

    /** The underlying platform socket (the native browser `WebSocket`). */
    get platformSocket(): WebSocket {
        return this._ws;
    }

    get readyState(): number {
        return this._ws.readyState;
    }

    send(data: string | ArrayBufferLike | ArrayBufferView): void {
        // The SDK base only ever sends JSON text frames; forward verbatim.
        this._ws.send(data as unknown as string);
    }

    close(code?: number, reason?: string): void {
        this._ws.close(code, reason);
    }

    on(event: string, listener: Listener): void {
        const wrapped = this._wrapListener(event, listener);
        this._listenersFor(event).set(listener, wrapped);
        this._ws.addEventListener(event, wrapped as EventListener);
    }

    off(event: string, listener: Listener): void {
        const byListener = this._listenerMap.get(event);
        if (!byListener) return;
        const wrapped = byListener.get(listener);
        if (wrapped) {
            byListener.delete(listener);
            this._ws.removeEventListener(event, wrapped as EventListener);
        }
    }

    once(event: string, listener: Listener): void {
        const onceListener: Listener = (...args) => {
            this.off(event, listener);
            listener(...args);
        };
        const wrapped = this._wrapListener(event, onceListener);
        this._listenersFor(event).set(listener, wrapped);
        this._ws.addEventListener(event, wrapped as EventListener);
    }

    private _listenersFor(event: string): Map<Listener, DOMEventHandler> {
        let map = this._listenerMap.get(event);
        if (!map) {
            map = new Map();
            this._listenerMap.set(event, map);
        }
        return map;
    }

    /**
     * Converts browser event objects into positional arguments matching the
     * {@link WebSocketLike} interface.
     */
    private _wrapListener(event: string, listener: Listener): DOMEventHandler {
        switch (event) {
            case "message":
                return (ev: Event) => {
                    const data = (ev as MessageEvent).data;
                    const isBinary = typeof data !== "string";
                    listener(data, isBinary);
                };
            case "close":
                return (ev: Event) => {
                    const close = ev as CloseEvent;
                    listener(close.code, close.reason);
                };
            case "error":
                return (ev: Event) => {
                    const errorEvent = ev as ErrorEvent;
                    const message =
                        errorEvent.message || errorEvent.error?.message || "WebSocket error";
                    const err = new Error(message);
                    if (errorEvent.error) {
                        err.cause = errorEvent.error;
                    }
                    listener(err);
                };
            case "open":
            default:
                return listener as DOMEventHandler;
        }
    }
}
