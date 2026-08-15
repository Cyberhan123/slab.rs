/**
 * Streaming layer for the harness protocol.
 *
 * The harness server pushes agent events as JSON-RPC notifications
 * (`turn/started`, `item/*`, `turn/completed`, `error`). This module converts
 * each notification into 0..N AI-SDK `UIMessageChunk`s, mirroring the
 * OpenAI-Responses `convertEvent` state machine but keyed on harness item ids
 * (which are stable within a turn).
 *
 * Tool calls are finalized via `item/completed` → `tool-input-available` followed
 * by `tool-output-available` (result) or `tool-output-error` (failure). The
 * approval-request notifications also emit `tool-input-available` so the card
 * shows the pending command/changes; approval status is tracked out-of-band by
 * the conversation controller. `tool-input-delta` is intentionally avoided — it
 * requires a preceding `tool-input-start` we never emit.
 *
 * Terminal detection: a failed turn emits an `error` notification with NO
 * subsequent `turn/completed`, so both `turn/completed` and `error` are treated
 * as turn-terminal by {@link isTerminalNotification}.
 */

import type { UIMessageChunk } from "ai"

import { HARNESS_NOTIFICATION } from "./types"
import { toolItemFields } from "./turn-items"
import type {
  AgentMessageDeltaParams,
  CommandExecutionRequestApprovalParams,
  ErrorParams,
  FileChangeRequestApprovalParams,
  ItemCompletedParams,
  ItemStartedParams,
  JsonRpcNotification,
  ReasoningSummaryTextDeltaParams,
  ReasoningTextDeltaParams,
  ServerNotification,
  TurnCompletedParams,
  TurnItem,
} from "./types"

/** Every server → client notification method we model. */
const HARNESS_NOTIFICATION_METHODS = new Set<string>(Object.values(HARNESS_NOTIFICATION))

/**
 * Narrow a raw {@link JsonRpcNotification} into the typed {@link ServerNotification}
 * union, or `null` for methods we don't model. Params are trusted from the
 * server (no runtime validation); unrecognized methods are dropped.
 */
export function coerceServerNotification(notification: JsonRpcNotification): ServerNotification | null {
  if (!HARNESS_NOTIFICATION_METHODS.has(notification.method)) return null
  return {
    method: notification.method,
    params: notification.params ?? {},
  } as unknown as ServerNotification
}

/** Mutable per-turn streaming state carried across `convertNotification` calls. */
export interface StreamState {
  finished: boolean
  /** Item ids with an open text part. */
  openText: Set<string>
  /** Item ids with an open reasoning part. */
  openReasoning: Set<string>
}

export function createStreamState(): StreamState {
  return { finished: false, openText: new Set(), openReasoning: new Set() }
}

function openText(state: StreamState, itemId: string): UIMessageChunk[] {
  if (state.openText.has(itemId)) return []
  state.openText.add(itemId)
  return [{ id: itemId, type: "text-start" }]
}

function closeText(state: StreamState, itemId: string): UIMessageChunk[] {
  if (!state.openText.has(itemId)) return []
  state.openText.delete(itemId)
  return [{ id: itemId, type: "text-end" }]
}

function openReasoning(state: StreamState, itemId: string): UIMessageChunk[] {
  if (state.openReasoning.has(itemId)) return []
  state.openReasoning.add(itemId)
  return [{ id: itemId, type: "reasoning-start" }]
}

function closeReasoning(state: StreamState, itemId: string): UIMessageChunk[] {
  if (!state.openReasoning.has(itemId)) return []
  state.openReasoning.delete(itemId)
  return [{ id: itemId, type: "reasoning-end" }]
}

function finishChunks(state: StreamState, reason: "stop" | "error" = "stop"): UIMessageChunk[] {
  if (state.finished) return []
  const chunks: UIMessageChunk[] = []
  for (const itemId of state.openReasoning) chunks.push({ id: itemId, type: "reasoning-end" })
  for (const itemId of state.openText) chunks.push({ id: itemId, type: "text-end" })
  state.openReasoning.clear()
  state.openText.clear()
  chunks.push({ type: "finish-step" }, { finishReason: reason, type: "finish" })
  state.finished = true
  return chunks
}

/**
 * Build `tool-input-available` (and, when finalized, `tool-output-available` /
 * `tool-output-error`) chunks for a tool-like item. Mirrors the AI-SDK tool-part
 * lifecycle so the assembled `type: "tool-<name>"` part carries its parameters
 * AND its result/error — letting the tool card render a Completed/Error badge
 * with the result, instead of folding everything into the input.
 *
 * Field derivation is delegated to {@link toolItemFields} (shared with the
 * history path in `turn-items.ts`) so live + history cannot drift on how a
 * command/mcp/file/websearch item maps to input/output/error.
 */
function toolChunksFromItem(item: TurnItem): UIMessageChunk[] {
  const fields = toolItemFields(item)
  if (!fields) return []
  const chunks: UIMessageChunk[] = [
    {
      input: fields.input,
      toolCallId: item.id,
      toolName: fields.toolName,
      type: "tool-input-available",
    },
  ]
  if (fields.failed) {
    chunks.push({
      errorText: fields.errorText ?? "",
      toolCallId: item.id,
      type: "tool-output-error",
    })
  } else if (fields.output !== undefined) {
    chunks.push({
      output: fields.output,
      toolCallId: item.id,
      type: "tool-output-available",
    })
  }
  return chunks
}

function handleItemStarted(state: StreamState, params: ItemStartedParams): UIMessageChunk[] {
  const { item } = params
  if (item.type === "agentMessage") {
    // The assistant's main message is starting. Close any reasoning part that is
    // still open so its "Thinking..." indicator stops immediately — even when the
    // server omits an explicit `item/completed(reasoning)` and jumps straight to
    // the agent message. Idempotent: a no-op when `openReasoning` is already empty.
    const chunks: UIMessageChunk[] = []
    for (const reasoningId of state.openReasoning) {
      chunks.push({ id: reasoningId, type: "reasoning-end" })
    }
    state.openReasoning.clear()
    return chunks.concat(openText(state, item.id))
  }
  if (item.type === "reasoning") return openReasoning(state, item.id)
  return []
}

function handleItemCompleted(state: StreamState, params: ItemCompletedParams): UIMessageChunk[] {
  const { item } = params
  if (item.type === "agentMessage") return closeText(state, item.id)
  if (item.type === "reasoning") return closeReasoning(state, item.id)
  return toolChunksFromItem(item)
}

function handleAgentMessageDelta(
  state: StreamState,
  params: AgentMessageDeltaParams,
): UIMessageChunk[] {
  return openText(state, params.itemId).concat({
    delta: params.delta,
    id: params.itemId,
    type: "text-delta",
  })
}

function handleReasoningDelta(
  state: StreamState,
  params: ReasoningTextDeltaParams | ReasoningSummaryTextDeltaParams,
): UIMessageChunk[] {
  return openReasoning(state, params.itemId).concat({
    delta: params.delta,
    id: params.itemId,
    type: "reasoning-delta",
  })
}

function handleRequestApproval(
  params: CommandExecutionRequestApprovalParams | FileChangeRequestApprovalParams,
): UIMessageChunk[] {
  const isCommand = "command" in params
  const input = isCommand
    ? { command: params.command, cwd: params.cwd }
    : { changes: params.changes }
  return [
    {
      input,
      toolCallId: params.itemId,
      toolName: isCommand ? "commandExecution" : "fileChange",
      type: "tool-input-available",
    },
  ]
}

function handleTurnCompleted(state: StreamState, params: TurnCompletedParams): UIMessageChunk[] {
  const failed = params.turn.status === "failed" || Boolean(params.turn.error)
  return finishChunks(state, failed ? "error" : "stop")
}

function handleError(params: ErrorParams): UIMessageChunk[] {
  return [{ errorText: params.message || "harness error", type: "error" }]
}

/**
 * Convert one harness notification into 0..N `UIMessageChunk`s, mutating `state`
 * to track open text/reasoning parts and the terminal finish.
 */
export function convertNotification(
  notification: ServerNotification,
  state: StreamState,
): UIMessageChunk[] {
  switch (notification.method) {
    case HARNESS_NOTIFICATION.THREAD_STARTED:
    case HARNESS_NOTIFICATION.TURN_STARTED:
    case HARNESS_NOTIFICATION.ITEM_COMMAND_EXECUTION_OUTPUT_DELTA:
    case HARNESS_NOTIFICATION.ITEM_FILE_CHANGE_OUTPUT_DELTA:
      // Lifecycle no-ops; shell/file output deltas are not progressively chunked
      // (the finalized call arrives via `item/completed`).
      return []
    case HARNESS_NOTIFICATION.ITEM_STARTED:
      return handleItemStarted(state, notification.params)
    case HARNESS_NOTIFICATION.ITEM_COMPLETED:
      return handleItemCompleted(state, notification.params)
    case HARNESS_NOTIFICATION.ITEM_AGENT_MESSAGE_DELTA:
      return handleAgentMessageDelta(state, notification.params)
    case HARNESS_NOTIFICATION.ITEM_REASONING_TEXT_DELTA:
    case HARNESS_NOTIFICATION.ITEM_REASONING_SUMMARY_TEXT_DELTA:
      return handleReasoningDelta(state, notification.params)
    case HARNESS_NOTIFICATION.ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL:
    case HARNESS_NOTIFICATION.ITEM_FILE_CHANGE_REQUEST_APPROVAL:
      return handleRequestApproval(notification.params)
    case HARNESS_NOTIFICATION.TURN_COMPLETED:
      return handleTurnCompleted(state, notification.params)
    case HARNESS_NOTIFICATION.ERROR:
      return handleError(notification.params)
    case HARNESS_NOTIFICATION.ACCOUNT_UPDATED:
    case HARNESS_NOTIFICATION.ACCOUNT_LOGIN_COMPLETED:
      return []
    case HARNESS_NOTIFICATION.MODEL_LOAD_DELTA:
    case HARNESS_NOTIFICATION.MODEL_LOAD_COMPLETED:
      // Model load lifecycle is consumed out-of-band by the conversation controller
      // (transient `modelLoad` state), not as AI-SDK message parts.
      return []
    case HARNESS_NOTIFICATION.CONTEXT_COMPACTING:
    case HARNESS_NOTIFICATION.CONTEXT_COMPACTED:
      // Compaction lifecycle is consumed out-of-band by the conversation controller
      // (session-scoped `compactionMarkers` state), not as AI-SDK message parts.
      return []
    default:
      // Exhaustiveness guard for future notification variants.
      return []
  }
}

/** A notification that ends the current turn's stream (`turn/completed` or `error`). */
export function isTerminalNotification(notification: ServerNotification): boolean {
  return (
    notification.method === HARNESS_NOTIFICATION.TURN_COMPLETED ||
    notification.method === HARNESS_NOTIFICATION.ERROR
  )
}
