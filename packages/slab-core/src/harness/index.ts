/**
 * Harness protocol module — slab-owned `/v1/agents/harness` WebSocket JSON-RPC
 * control plane for the assistant page.
 *
 * - {@link HarnessClient}: persistent JSON-RPC WS client (initialize, request
 *   correlation, notification dispatch).
 * - {@link HarnessChatTransport}: `ChatTransport<UIMessage>` driving live turns.
 * - {@link turnItemsToMessages}: restore resumed `TurnItem`s into `UIMessage[]`.
 * - {@link convertNotification}: harness notification → `UIMessageChunk[]`.
 */

export { HarnessClient, harnessWebSocketUrl } from "./harness-client"
export type {
  HarnessChatTransportOptions,
} from "./harness-transport"
export { HarnessChatTransport } from "./harness-transport"
export { turnItemsToMessages } from "./turn-items"
export {
  coerceServerNotification,
  convertNotification,
  createStreamState,
  isTerminalNotification,
} from "./stream"
export type { StreamState } from "./stream"
export { classifyJsonRpcMessage, nextRequestId } from "./json-rpc"
export type { ClassifiedJsonRpc } from "./json-rpc"
export * from "./types"
