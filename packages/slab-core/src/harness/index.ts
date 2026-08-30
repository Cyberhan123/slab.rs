/**
 * Harness protocol module — slab-owned `/v1/agents/harness` WebSocket JSON-RPC
 * control plane for the assistant page.
 *
 * - {@link HarnessClient}: persistent JSON-RPC WS client (initialize, request
 *   correlation, notification dispatch).
 * - {@link HarnessChatTransport}: `ChatTransport<UIMessage>` driving live turns.
 * - {@link ConversationController}: framework-free conversation state machine
 *   (restore, approvals, compaction, plan mode) exposed as an external store.
 * - {@link turnItemsToMessages}: restore resumed `TurnItem`s into `UIMessage[]`.
 * - {@link convertNotification}: harness notification → `UIMessageChunk[]`.
 */

export { HarnessClient, harnessWebSocketUrl } from "./harness-client"
export type {
  HarnessChatTransportOptions,
} from "./harness-transport"
export { HarnessChatTransport } from "./harness-transport"
export { turnItemsToMessages } from "./turn-items"
export { buildTurnInput } from "./turn-input"
export {
  ConversationController,
  MAX_RESTORE_ATTEMPTS,
  RESTORE_BACKOFF_MS,
  buildUserMessageTurnIndex,
} from "./conversation-controller"
export type {
  ActionError,
  ApprovalRequest,
  ApprovalStatus,
  BackgroundTaskInfo,
  CompactionMarker,
  CompactionMode,
  CompactionPhase,
  ConversationControllerOptions,
  ConversationState,
  ModelLoadState,
  ThreadStatusString,
  TurnSendOptions,
} from "./conversation-controller"
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
