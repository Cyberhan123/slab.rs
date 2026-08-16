/**
 * Harness protocol types.
 *
 * The payload contract is GENERATED from the authoritative Rust types in
 * `crates/slab-proto` + `crates/slab-agent` (see `./generated/index.ts`,
 * refreshed by `bun run gen:harness`). This module keeps the pieces codegen
 * does not own: the generic JSON-RPC 2.0 envelope (from `slab-jsonrpc`) and
 * the client-side fallback aliases. Wire fields are camelCase; optional
 * fields are omitted on the wire (not `null`).
 *
 * The harness control plane is a WebSocket JSON-RPC 2.0 connection at
 * `/v1/agents/harness?token=<sessionId>`. The client sends requests, the server
 * responds, and the server pushes agent events as notifications.
 */

export * from "./constants"
export * from "./generated/index"

// ── JSON-RPC 2.0 envelope ───────────────────────────────────────────────────

export type RequestId = string | number

export interface JsonRpcRequest<P = unknown> {
  jsonrpc: "2.0"
  id: RequestId
  method: string
  params?: P
}

export interface JsonRpcResponse {
  jsonrpc: "2.0"
  id: RequestId
  result: unknown
}

export interface JsonRpcErrorResponse {
  jsonrpc: "2.0"
  id: RequestId
  error: JsonRpcErrorBody
}

export interface JsonRpcErrorBody {
  code: number
  message: string
  data?: unknown
}

export interface JsonRpcNotification<P = unknown> {
  jsonrpc: "2.0"
  method: string
  params?: P
}

export type JsonRpcMessage = JsonRpcRequest | JsonRpcResponse | JsonRpcErrorResponse | JsonRpcNotification

// ── Client-side aliases (not part of the generated wire contract) ───────────

/**
 * Turn status — an open string set on the wire: `completed` | `interrupted` |
 * `failed` | `inProgress` (plus PascalCase aliases accepted on decode).
 */
export type TurnStatus = string

/** A notification whose `method` we don't model explicitly. */
export interface UnknownNotification {
  method: string
  params?: unknown
}
