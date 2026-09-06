/**
 * Slab chat transport for the harness JSON-RPC protocol.
 *
 * Shares a long-lived {@link HarnessClient}: on each `sendMessages` it ensures a
 * thread is bound (`thread/start` for a fresh session), subscribes to the live
 * turn's notifications, fires `turn/start`, and converts the harness
 * `item/*` / `turn/*` / `error` notifications into AI-SDK `UIMessageChunk`s until
 * the turn terminates (`turn/completed` or `error`).
 *
 * Replay vs. live: `thread/resume` (restore) replays historical events, which
 * the client ignores (it restores from the resume `result.thread` directly).
 * During an active turn we additionally guard against a straggling replay event
 * by only routing notifications whose `turnId` is newer than the threshold
 * captured at turn start (non-numeric `turnId`, terminal events, and
 * thread-matched errors always pass).
 */

import {
  type ChatTransport,
  type UIMessage,
  type UIMessageChunk,
  createUIMessageStream,
} from "ai"

import type {
  JsonRpcNotification,
  PermissionMode,
  ReasoningEffort,
  TurnStartParams,
  UserInput,
} from "@slab/api/harness"

import { getNotifier } from "../platform/notifications"

import type { HarnessClient } from "./harness-client"
import { MAX_RESTORE_ATTEMPTS, RESTORE_BACKOFF_MS } from "./conversation-controller"
import {
  coerceServerNotification,
  convertNotification,
  createStreamState,
  isTerminalNotification,
} from "./stream"
import { buildTurnInput } from "./turn-input"

export interface HarnessChatTransportOptions {
  /** The shared, long-lived harness client (owns the WS + bound thread). */
  client: HarnessClient
  /** Model id sent on `turn/start` (defaults to "slab-llama"). */
  model?: string
  /**
   * Local AI-SDK stream lifecycle → ConversationController. All optional
   * (tests and non-controller hosts omit them).
   *
   * - `onLocalStreamBegin`: a locally-initiated send is starting (covers the
   *   pre-`turn/start` "submitted" window — the pane-remount race happens
   *   there). Drives the mid-run restoreVersion suppression.
   * - `onLocalTurnStarted`: the server ACCEPTED the turn (`turn/start`
   *   resolved). Advances `turnStartSeq` — the pane's failed-send re-stage
   *   decision keys off it. Never fires for a rejected `turn/start`.
   * - `onLocalStreamEnd`: the stream finished (any outcome), un-sticking the
   *   suppression flag on failed sends.
   */
  onLocalStreamBegin?: () => void
  onLocalTurnStarted?: (threadId: string) => void
  onLocalStreamEnd?: () => void
}

/** Read the reasoning-effort selector carried via `sendMessage({ metadata })`. */
function readEffort(metadata: unknown): ReasoningEffort | undefined {
  if (!metadata || typeof metadata !== "object") return undefined
  const effort = (metadata as { effort?: unknown }).effort
  if (
    effort === "off" ||
    effort === "low" ||
    effort === "medium" ||
    effort === "high" ||
    effort === "xhigh"
  ) {
    return effort
  }
  return undefined
}

/** Read the per-session permission-mode selector carried via `sendMessage({ metadata })`. */
function readPermissionMode(metadata: unknown): PermissionMode | undefined {
  if (!metadata || typeof metadata !== "object") return undefined
  const mode = (metadata as { permissionMode?: unknown }).permissionMode
  if (
    mode === "request_approval" ||
    mode === "approve_for_me" ||
    mode === "full_control" ||
    mode === "custom"
  ) {
    return mode
  }
  return undefined
}

/** Read the built-in agent type carried via `sendMessage({ metadata })` (`"plan"` when plan mode is on). */
function readAgentType(metadata: unknown): "plan" | undefined {
  if (!metadata || typeof metadata !== "object") return undefined
  const agentType = (metadata as { agentType?: unknown }).agentType
  return agentType === "plan" ? "plan" : undefined
}

export class HarnessChatTransport<UI_MESSAGE extends UIMessage> implements ChatTransport<UI_MESSAGE> {
  private readonly client: HarnessClient
  private readonly model: string
  private readonly onLocalStreamBegin?: () => void
  private readonly onLocalTurnStarted?: (threadId: string) => void
  private readonly onLocalStreamEnd?: () => void

  constructor(options: HarnessChatTransportOptions) {
    this.client = options.client
    this.model = options.model ?? "slab-llama"
    this.onLocalStreamBegin = options.onLocalStreamBegin
    this.onLocalTurnStarted = options.onLocalTurnStarted
    this.onLocalStreamEnd = options.onLocalStreamEnd
  }

  async sendMessages(options: {
    messages: UI_MESSAGE[]
    abortSignal?: AbortSignal
    /** Carries `effort`/`permissionMode`/`agentType` from
     * `sendMessage({ metadata })` — the AI SDK hands the per-request metadata
     * to custom transports under `metadata` (ChatRequestOptions), NOT
     * `requestMetadata` (that key exists only on HttpChatTransport's
     * `prepareSendMessagesRequest`). */
    metadata?: unknown
  }): Promise<ReadableStream<UIMessageChunk>> {
    const input = buildTurnInput(options.messages)
    // The per-request composer metadata (effort/permissionMode/agentType)
    // arrives either as the ChatRequestOptions `metadata` (sendMessage's
    // second argument) or attached to the submitted user message — the
    // one-argument `sendMessage({ text, metadata })` form stores it on the
    // MESSAGE only. Prefer the request-level value, fall back to the newest
    // user message's.
    const lastUserMetadata = [...options.messages]
      .reverse()
      .find((message) => message.role === "user")?.metadata
    const metadata = options.metadata ?? lastUserMetadata
    const effort = readEffort(metadata)
    const permissionMode = readPermissionMode(metadata)
    const agentType = readAgentType(metadata)

    return createUIMessageStream({
      execute: async ({ writer }) => {
        // Local-stream lifecycle: begin BEFORE any await (the pane-remount
        // race lives in the pre-turn/start "submitted" window), end once the
        // stream settles on any outcome.
        this.onLocalStreamBegin?.()
        try {
          await this.openWithRetry()

          // Bind a thread if none is bound yet (fresh session, no prior resume).
          if (this.client.currentThreadId === null) {
            const started = await this.client.threadStart({ model: this.model })
            this.client.currentThreadId = started.thread.id
          }
          const threadId = this.client.currentThreadId
          if (!threadId) {
            writer.write({ errorText: "no harness thread bound", type: "error" })
            writer.write({ finishReason: "error", type: "finish" })
            return
          }

          const threshold = this.client.lastTurnIndex
          const state = createStreamState()
          let finished = false

          await new Promise<void>((resolve) => {
            const done = () => {
              if (finished) return
              finished = true
              unsubscribe()
              resolve()
            }

            const unsubscribe = this.client.onNotification(
              (notification: JsonRpcNotification) => {
                if (finished) return
                const params = (notification.params ?? {}) as { threadId?: string; turnId?: string }
                // Ignore notifications for a different thread on the shared socket.
                if (params.threadId !== undefined && params.threadId !== threadId) return

                const serverNotif = coerceServerNotification(notification)
                if (!serverNotif) return

                const terminal = isTerminalNotification(serverNotif)
                // Drop replayed history (turnId at or below the threshold) unless
                // terminal or carrying a non-numeric turnId.
                if (!terminal) {
                  const turnNum = Number(params.turnId)
                  if (!Number.isNaN(turnNum) && turnNum <= threshold) return
                }

                for (const chunk of convertNotification(serverNotif, state)) {
                  writer.write(chunk)
                }
                if (terminal) done()
              },
            )

            // Fire the turn; its events arrive via the subscription above. The
            // `turnStart` response is not awaited (its `turn.id` is hardcoded and
            // uninformative) — a rejection is surfaced as an error + finish.
            const turnParams: TurnStartParams = {
              threadId,
              // Always send at least a text part so the turn has well-formed input.
              input:
                input.length > 0
                  ? input
                  : ([{ text: "", textElements: [], type: "text" }] satisfies UserInput[]),
              model: this.model,
            }
            if (effort) turnParams.effort = effort
            if (permissionMode) turnParams.permissionMode = permissionMode
            if (agentType) turnParams.agentType = agentType
            this.client
              .turnStart(turnParams)
              // Ack the ACCEPTED turn (advances the controller's turnStartSeq).
              // A rejection takes the catch below and never acks — the pane
              // then knows the turn never started server-side and may
              // re-stage the draft.
              .then(() => {
                this.onLocalTurnStarted?.(threadId)
              })
              .catch((error) => {
                if (finished) return
                const message = error instanceof Error ? error.message : "turn failed"
                // Surface a failed turn (e.g. model-load failure) visibly instead
                // of a silent empty bubble — the part stream has no error slot.
                getNotifier().error(message)
                writer.write({ errorText: message, type: "error" })
                writer.write({ finishReason: "error", type: "finish" })
                done()
              })

            options.abortSignal?.addEventListener(
              "abort",
              () => {
                if (finished) return
                // Local stream teardown only — do NOT send `turn/interrupt` here.
                // The Stop control already routes the authoritative interrupt
                // through ConversationController.interrupt (both wired in the
                // Sender's onStop), and this abort listener also fires for
                // UNMOUNT aborts (pane remounts on restore-version bumps) where
                // the server turn must keep running. A duplicate interrupt here
                // raced the teardown's terminal-status SQL write and could
                // strand the thread row on "interrupting" forever.
                if (!state.finished) {
                  writer.write({ finishReason: "stop", type: "finish" })
                }
                done()
              },
              { once: true },
            )
          })
        } finally {
          this.onLocalStreamEnd?.()
        }
      },
      onError: (error) => (error instanceof Error ? error.message : "stream error"),
    })
  }

  /**
   * `open()` with the restore path's backed-off retries: `open()` is
   * single-flight on the client, but the send path used to join only the
   * FIRST attempt's promise — when that attempt failed while a racing
   * restore's retry later succeeded, the send (and with it the claimed
   * draft) died even though the socket came up moments later.
   */
  private async openWithRetry(): Promise<void> {
    let lastError: unknown
    for (let attempt = 1; attempt <= MAX_RESTORE_ATTEMPTS; attempt += 1) {
      try {
        await this.client.open()
        return
      } catch (openError) {
        lastError = openError
        if (attempt < MAX_RESTORE_ATTEMPTS) {
          await new Promise((resolve) => setTimeout(resolve, RESTORE_BACKOFF_MS * attempt))
        }
      }
    }
    throw lastError
  }

  reconnectToStream(): Promise<ReadableStream<UIMessageChunk> | null> {
    // Harness has no server-side resumable stream; `null` tells `useChat` there
    // is nothing to reconnect. Reload re-runs `thread/resume` via the hook.
    return Promise.resolve(null)
  }
}
