/**
 * Persistent JSON-RPC 2.0 WebSocket client for the `/v1/agents/harness` control
 * plane.
 *
 * One client owns one socket for one slab session: it opens the connection,
 * completes the mandatory `initialize` handshake, correlates requests by id,
 * and dispatches server-pushed notifications (the agent event stream) to
 * subscribers. Threads are harness-local & socket-scoped, so the client tracks
 * the bound `currentThreadId` plus `lastTurnIndex` (used to separate replayed
 * history from live-turn events on the same socket).
 *
 * The client does NOT auto-reconnect — on an unexpected close it transitions to
 * `"closed"` and emits, leaving re-`open()` + re-resume to the owning hook.
 */

import { SERVER_BASE_URL } from "@slab/api/config"

import { classifyJsonRpcMessage, nextRequestId } from "./json-rpc"
import type {
  ApprovalResolveParams,
  ApprovalResolveResult,
  InitializeResult,
  JsonRpcNotification,
  JsonRpcRequest,
  ModelListParams,
  ModelListResult,
  RequestId,
  ShutdownParams,
  ShutdownResult,
  SkillsListParams,
  SkillsListResult,
  ThreadArchiveParams,
  ThreadArchiveResult,
  ThreadForkParams,
  ThreadForkResult,
  ThreadListParams,
  ThreadListResult,
  ThreadResumeParams,
  ThreadResumeResult,
  ThreadRollbackParams,
  ThreadRollbackResult,
  ThreadStartParams,
  ThreadStartResult,
  TurnInterruptParams,
  TurnInterruptResult,
  TurnStartParams,
  TurnStartResult,
  WorkspaceMigrateParams,
  WorkspaceMigrateResult,
} from "./types"
import { HARNESS_METHOD } from "./types"

export interface HarnessClientOptions {
  /** Slab session id, carried on the WS URL as `?token=`. */
  sessionId: string
  /** Override the server base URL (defaults to `SERVER_BASE_URL`). */
  baseURL?: string
  /** Inject a `WebSocket` constructor (tests pass a fake). */
  WebSocketCtor?: typeof WebSocket
}

export type HarnessClientStatus = "idle" | "opening" | "ready" | "closed"

export type HarnessNotificationHandler = (notification: JsonRpcNotification) => void

export type HarnessStatusListener = (status: HarnessClientStatus) => void

/** Cap on how long we wait for the WS handshake before failing `open()`. */
const WS_OPEN_TIMEOUT_MS = 5000
/** Cap on how long we wait for any single JSON-RPC response. */
const REQUEST_TIMEOUT_MS = 30000

interface PendingRequest {
  resolve: (value: unknown) => void
  reject: (error: Error) => void
  timer: ReturnType<typeof setTimeout>
}

/** Build the harness WS URL: `ws(s)://<origin>/v1/agents/harness?token=<sessionId>`. */
export function harnessWebSocketUrl(baseURL: string, sessionId: string): string {
  const url = new URL(baseURL)
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:"
  url.pathname = "/v1/agents/harness"
  url.search = `token=${encodeURIComponent(sessionId)}`
  url.hash = ""
  return url.toString()
}

function parseFrame(data: unknown): unknown {
  if (typeof data !== "string") return null
  try {
    return JSON.parse(data)
  } catch {
    return null
  }
}

export class HarnessClient {
  private readonly sessionId: string
  private readonly baseURL: string
  private readonly WebSocketCtor: typeof WebSocket
  private socket: WebSocket | null = null
  private status: HarnessClientStatus = "idle"
  private openPromise: Promise<void> | null = null
  private readonly pending = new Map<RequestId, PendingRequest>()
  private readonly notificationHandlers = new Set<HarnessNotificationHandler>()
  private readonly statusListeners = new Set<HarnessStatusListener>()

  /**
   * The harness thread id currently bound on this socket (`thread/start` or
   * `thread/resume`). Used as the `threadId` for `turn/start` etc.
   */
  currentThreadId: string | null = null
  /**
   * Highest numeric `turnId` seen on the current thread. Live-turn events have
   * `turnId > lastTurnIndex`; replayed history has `turnId <= lastTurnIndex`.
   */
  lastTurnIndex = -1

  constructor(options: HarnessClientOptions) {
    this.sessionId = options.sessionId
    this.baseURL = options.baseURL ?? SERVER_BASE_URL
    this.WebSocketCtor = options.WebSocketCtor ?? WebSocket
  }

  getStatus(): HarnessClientStatus {
    return this.status
  }

  /** Subscribe to connection status changes. Returns an unsubscribe fn. */
  onStatusChange(listener: HarnessStatusListener): () => void {
    this.statusListeners.add(listener)
    return () => this.statusListeners.delete(listener)
  }

  /** Subscribe to inbound server notifications. Returns an unsubscribe fn. */
  onNotification(handler: HarnessNotificationHandler): () => void {
    this.notificationHandlers.add(handler)
    return () => this.notificationHandlers.delete(handler)
  }

  /** Connect (if needed) and complete the `initialize` handshake. Idempotent. */
  async open(): Promise<void> {
    if (this.status === "ready") return
    if (this.openPromise) return this.openPromise
    this.openPromise = this.runOpen()
    try {
      await this.openPromise
    } finally {
      this.openPromise = null
    }
  }

  private async runOpen(): Promise<void> {
    this.setStatus("opening")
    const socket = new this.WebSocketCtor(harnessWebSocketUrl(this.baseURL, this.sessionId))
    this.socket = socket
    await this.awaitOpen(socket)
    socket.addEventListener("message", this.handleMessage)
    socket.addEventListener("close", this.handleClose)
    // Mandatory handshake — every other method is rejected until this returns.
    await this.sendRequest<InitializeResult>(HARNESS_METHOD.INITIALIZE, {
      clientInfo: { name: "slab-desktop", version: "1.0" },
    })
    this.setStatus("ready")
  }

  private awaitOpen(socket: WebSocket): Promise<void> {
    return new Promise<void>((resolve, reject) => {
      if (socket.readyState === 1) {
        resolve()
        return
      }
      const cleanup = () => {
        clearTimeout(timer)
        socket.removeEventListener("open", onOpen)
        socket.removeEventListener("error", onError)
        socket.removeEventListener("close", onClose)
      }
      const timer = setTimeout(() => {
        cleanup()
        reject(new Error("harness websocket open timed out"))
      }, WS_OPEN_TIMEOUT_MS)
      const onOpen = () => {
        cleanup()
        resolve()
      }
      const onError = () => {
        cleanup()
        reject(new Error("harness websocket error"))
      }
      const onClose = () => {
        cleanup()
        reject(new Error("harness websocket closed before opening"))
      }
      socket.addEventListener("open", onOpen)
      socket.addEventListener("error", onError)
      socket.addEventListener("close", onClose)
    })
  }

  private readonly handleMessage = (event: { data?: unknown }) => {
    const classified = classifyJsonRpcMessage(parseFrame(event.data))
    if (classified.kind === "response") {
      this.settle(classified.message.id, classified.message.result)
    } else if (classified.kind === "error") {
      this.settle(
        classified.message.id,
        undefined,
        new Error(classified.message.error.message),
      )
    } else if (classified.kind === "notification") {
      for (const handler of this.notificationHandlers) handler(classified.message)
    }
    // Inbound requests / invalid frames are ignored (the server does not send
    // requests to the client).
  }

  private readonly handleClose = () => {
    for (const entry of this.pending.values()) {
      clearTimeout(entry.timer)
      entry.reject(new Error("harness socket closed"))
    }
    this.pending.clear()
    this.socket = null
    this.setStatus("closed")
  }

  private settle(id: RequestId, value: unknown, error?: Error): void {
    const entry = this.pending.get(id)
    if (!entry) return
    this.pending.delete(id)
    clearTimeout(entry.timer)
    if (error) entry.reject(error)
    else entry.resolve(value)
  }

  /**
   * Send a JSON-RPC request and await its `result`. Ensures the socket is open
   * (and initialized) first, except for `initialize` itself (sent by `runOpen`).
   */
  sendRequest<T>(method: string, params?: unknown): Promise<T> {
    return this.queueRequest<T>(method, params)
  }

  private async queueRequest<T>(method: string, params?: unknown): Promise<T> {
    if (this.status !== "ready" && method !== HARNESS_METHOD.INITIALIZE) {
      await this.open()
    }
    const socket = this.socket
    if (!socket || socket.readyState !== 1) {
      throw new Error("harness socket is not open")
    }
    const id = nextRequestId()
    const frame: JsonRpcRequest = {
      jsonrpc: "2.0",
      id,
      method,
      ...(params === undefined ? {} : { params }),
    }
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.settle(id, undefined, new Error(`harness request timed out: ${method}`))
      }, REQUEST_TIMEOUT_MS)
      this.pending.set(id, {
        resolve: resolve as (value: unknown) => void,
        reject,
        timer,
      })
      socket.send(JSON.stringify(frame))
    })
  }

  // ── Method wrappers ──────────────────────────────────────────────────────

  threadStart(params: ThreadStartParams): Promise<ThreadStartResult> {
    return this.sendRequest(HARNESS_METHOD.THREAD_START, params)
  }

  threadResume(params: ThreadResumeParams = {}): Promise<ThreadResumeResult> {
    return this.sendRequest(HARNESS_METHOD.THREAD_RESUME, params)
  }

  turnStart(params: TurnStartParams): Promise<TurnStartResult> {
    return this.sendRequest(HARNESS_METHOD.TURN_START, params)
  }

  turnInterrupt(params: TurnInterruptParams): Promise<TurnInterruptResult> {
    return this.sendRequest(HARNESS_METHOD.TURN_INTERRUPT, params)
  }

  approvalResolve(params: ApprovalResolveParams): Promise<ApprovalResolveResult> {
    return this.sendRequest(HARNESS_METHOD.APPROVAL_RESOLVE, params)
  }

  shutdown(params: ShutdownParams): Promise<ShutdownResult> {
    return this.sendRequest(HARNESS_METHOD.SHUTDOWN, params)
  }

  threadFork(params: ThreadForkParams): Promise<ThreadForkResult> {
    return this.sendRequest(HARNESS_METHOD.THREAD_FORK, params)
  }

  threadRollback(params: ThreadRollbackParams): Promise<ThreadRollbackResult> {
    return this.sendRequest(HARNESS_METHOD.THREAD_ROLLBACK, params)
  }

  threadArchive(params: ThreadArchiveParams): Promise<ThreadArchiveResult> {
    return this.sendRequest(HARNESS_METHOD.THREAD_ARCHIVE, params)
  }

  threadList(params: ThreadListParams = {}): Promise<ThreadListResult> {
    return this.sendRequest(HARNESS_METHOD.THREAD_LIST, params)
  }

  modelList(params: ModelListParams = {}): Promise<ModelListResult> {
    return this.sendRequest(HARNESS_METHOD.MODEL_LIST, params)
  }

  skillsList(params: SkillsListParams = {}): Promise<SkillsListResult> {
    return this.sendRequest(HARNESS_METHOD.SKILLS_LIST, params)
  }

  workspaceMigrate(params: WorkspaceMigrateParams = {}): Promise<WorkspaceMigrateResult> {
    return this.sendRequest(HARNESS_METHOD.WORKSPACE_MIGRATE, params)
  }

  /** Close the socket and reject any pending requests. */
  close(): void {
    this.handleClose()
    if (this.socket) {
      try {
        this.socket.close()
      } catch {
        // ignore
      }
    }
    this.socket = null
  }

  private setStatus(status: HarnessClientStatus): void {
    this.status = status
    for (const listener of this.statusListeners) listener(status)
  }
}
