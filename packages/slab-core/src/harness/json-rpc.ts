/**
 * JSON-RPC 2.0 wire helpers for the harness control plane.
 *
 * The server sends responses (to our requests) and notifications (agent events).
 * This module classifies inbound frames and mints request ids; the envelope
 * types live in {@link ./types}.
 */

import type {
  JsonRpcErrorResponse,
  JsonRpcNotification,
  JsonRpcRequest,
  JsonRpcResponse,
  RequestId,
} from "@slab/api/harness"

let nextId = 1

/** Monotonic JSON-RPC request id (integer). */
export function nextRequestId(): RequestId {
  const id = nextId
  nextId += 1
  return id
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null
}

export type ClassifiedJsonRpc =
  | { kind: "request"; message: JsonRpcRequest }
  | { kind: "response"; message: JsonRpcResponse }
  | { kind: "error"; message: JsonRpcErrorResponse }
  | { kind: "notification"; message: JsonRpcNotification }
  | { kind: "invalid" }

/**
 * Classify one inbound wire frame. A frame with `method` is a notification
 * (no `id`) or request (with `id`); a frame with `result`/`error` plus `id` is
 * a response to a prior request.
 */
export function classifyJsonRpcMessage(value: unknown): ClassifiedJsonRpc {
  if (!isObject(value) || value.jsonrpc !== "2.0") {
    return { kind: "invalid" }
  }
  if (typeof value.method === "string") {
    if (value.id !== undefined) {
      return { kind: "request", message: value as unknown as JsonRpcRequest }
    }
    return { kind: "notification", message: value as unknown as JsonRpcNotification }
  }
  if (value.error !== undefined && value.id !== undefined) {
    return { kind: "error", message: value as unknown as JsonRpcErrorResponse }
  }
  if (value.result !== undefined && value.id !== undefined) {
    return { kind: "response", message: value as unknown as JsonRpcResponse }
  }
  return { kind: "invalid" }
}
