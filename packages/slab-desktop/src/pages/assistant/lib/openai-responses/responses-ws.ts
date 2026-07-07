/**
 * Browser-friendly openai Responses WebSocket client.
 *
 * The SDK ships `ResponsesWS` (`openai/resources/responses/ws`), but it is
 * Node-only: it hard-`import`s the `ws` package and authenticates via handshake
 * `headers` — neither works in a browser (no Node `ws`; browsers can't set WS
 * headers). However `ResponsesWSBase<TSocket>` (`openai/resources/responses/ws`,
 * exported via the package's `./resources/*` map) is transport-agnostic. It owns
 * URL building, `send`, `.on(type,…)` / `stream()` dispatch, the send-queue, and
 * reconnect — only `_createSocket` is abstract. `ResponsesWS` is simply
 * `ResponsesWSBase<NodeWebSocket>`; this class is `ResponsesWSBase<BrowserWebSocket>`,
 * plugging in a native browser `WebSocket` via the vendored adapter.
 *
 * Auth + canonical mode: browsers can't set the `Authorization` header the base
 * computes (`_authHeaders`), so we carry the slab session id as `?token=` and
 * request the openai-protocol dialect via the `slab.responses` subprotocol
 * (server-side `agent_responses_get` gates canonical mode on it).
 *
 * The WS URL is derived by the base from the client's `baseURL`:
 * `${baseURL}/responses` with an http/ws→ws scheme swap. For slab that resolves
 * to `ws://<origin>/v1/agents/responses` — the server's existing WS upgrade path.
 */

/* eslint-disable no-underscore-dangle -- `this._client` is a `protected` field of
 * the SDK's `ResponsesWSBase`; there is no public accessor, so accessing it from
 * our `_createSocket` override requires the SDK's own underscore-prefixed name. */
import type OpenAI from "openai";
import {
    ResponsesWSBase,
    type ResponsesWSBaseOptions,
} from "openai/resources/responses/ws-base";

import { BrowserWebSocket } from "./browser-ws-adapter";

export class SlabResponsesWS extends ResponsesWSBase<BrowserWebSocket> {
    constructor(client: OpenAI, options?: ResponsesWSBaseOptions) {
        super(client, options);
        // The base does not auto-connect; mirror `ResponsesWS` and dial now.
        this._connectInitial();
    }

    protected _createSocket(url: URL, _authHeaders: Record<string, string>): BrowserWebSocket {
        // Ignore the bearer the base computed (browsers can't set WS headers).
        // Carry the slab session id (the SDK `apiKey`) as `?token=` and declare
        // canonical (openai-protocol) mode via the `slab.responses` subprotocol.
        if (this._client.apiKey) {
            url.searchParams.set("token", this._client.apiKey);
        }
        return new BrowserWebSocket(new WebSocket(url.toString(), "slab.responses"));
    }
}
