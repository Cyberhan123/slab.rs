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
import { toast } from "sonner"

import type { HarnessClient } from "./harness-client"
import {
  coerceServerNotification,
  convertNotification,
  createStreamState,
  isTerminalNotification,
} from "./stream"
import type {
  JsonRpcNotification,
  PermissionMode,
  ReasoningEffort,
  TurnStartParams,
  UserInput,
} from "./types"

export interface HarnessChatTransportOptions {
  /** The shared, long-lived harness client (owns the WS + bound thread). */
  client: HarnessClient
  /** Model id sent on `turn/start` (defaults to "slab-llama"). */
  model?: string
}

/**
 * Build the new turn's `input` from the latest user message: its text plus any
 * image attachments (AI-SDK `file` parts with an image media type), mapped to
 * harness `UserInput` variants. Non-image files are ignored here (no harness
 * upload path yet).
 */
function buildTurnInput(messages: UIMessage[]): UserInput[] {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i]
    if (message.role !== "user") continue
    const input: UserInput[] = []
    const text = message.parts
      .filter((part): part is { type: "text"; text: string } => part.type === "text")
      .map((part) => part.text)
      .join("")
      .trim()
    if (text) input.push({ type: "text", text, textElements: [] })
    for (const part of message.parts) {
      if (part.type !== "file") continue
      const file = part as { type: "file"; mediaType?: string; url: string }
      if (!file.mediaType?.startsWith("image")) continue
      input.push({ type: "image", imageUrl: file.url, detail: "auto" })
    }
    return input
  }
  return []
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

  constructor(options: HarnessChatTransportOptions) {
    this.client = options.client
    this.model = options.model ?? "slab-llama"
  }

  async sendMessages(options: {
    messages: UI_MESSAGE[]
    abortSignal?: AbortSignal
    /** Carries `effort` from `sendMessage({ metadata })`. */
    requestMetadata?: unknown
  }): Promise<ReadableStream<UIMessageChunk>> {
    const input = buildTurnInput(options.messages)
    const effort = readEffort(options.requestMetadata)
    const permissionMode = readPermissionMode(options.requestMetadata)
    const agentType = readAgentType(options.requestMetadata)

    return createUIMessageStream({
      execute: async ({ writer }) => {
        await this.client.open()

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

          const unsubscribe = this.client.onNotification((notification: JsonRpcNotification) => {
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
          })

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
            .catch((error) => {
              if (finished) return
              const message = error instanceof Error ? error.message : "turn failed"
              // Surface a failed turn (e.g. model-load failure) visibly instead
              // of a silent empty bubble — the part stream has no error slot.
              toast.error(message)
              writer.write({ errorText: message, type: "error" })
              writer.write({ finishReason: "error", type: "finish" })
              done()
            })

          options.abortSignal?.addEventListener(
            "abort",
            () => {
              if (finished) return
              // Best-effort interrupt; finish the stream regardless so the UI
              // does not hang waiting for a terminal notification.
              this.client.turnInterrupt({ threadId, turnId: "0" }).catch(() => {})
              if (!state.finished) {
                writer.write({ finishReason: "stop", type: "finish" })
              }
              done()
            },
            { once: true },
          )
        })
      },
      onError: (error) => (error instanceof Error ? error.message : "stream error"),
    })
  }

  reconnectToStream(): Promise<ReadableStream<UIMessageChunk> | null> {
    // Harness has no server-side resumable stream; `null` tells `useChat` there
    // is nothing to reconnect. Reload re-runs `thread/resume` via the hook.
    return Promise.resolve(null)
  }
}
