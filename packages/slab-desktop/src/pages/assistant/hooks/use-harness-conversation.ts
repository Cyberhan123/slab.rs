/**
 * Owns the harness control-plane lifecycle for the assistant page.
 *
 * Given the current slab session id + model, this hook:
 *   - creates a persistent {@link HarnessClient} (recreated when the session
 *     changes, closed on unmount),
 *   - opens it, completes the `initialize` handshake, and `thread/resume`s the
 *     conversation — projecting the returned `Thread` into `UIMessage[]` for
 *     `useChat`'s `initialMessages`,
 *   - exposes a {@link HarnessChatTransport} bound to the live client.
 *
 * A fresh session (no thread to resume) yields an empty message list and leaves
 * the thread unbound so the first `turn/start` lazily creates it.
 */

import { useEffect, useMemo, useState } from "react"
import type { UIMessage } from "ai"

import {
  HarnessChatTransport,
  HarnessClient,
  projectThread,
  type Thread,
} from "../lib/harness"

export interface HarnessConversation {
  /** Transport bound to the live client (always defined; safe to pass to `useChat`). */
  transport: HarnessChatTransport<UIMessage>
  /** Restored conversation, seeded as `useChat` `initialMessages`. */
  restoredMessages: UIMessage[]
  /** Harness thread id of the restored conversation (null for a fresh session). */
  restoredThreadId: string | null
  /** Session id once its conversation has been restored (for sidebar highlight). */
  activeConversation: string | undefined
  /** Bumped after each restore completes; used as a remount key for the chat pane. */
  restoreVersion: number
  /** True while connecting / restoring. */
  isHistoryLoading: boolean
  /** Restore error message, if any. */
  error: string | null
}

/** Highest numeric turn id in a thread (-1 when there are no turns). */
function computeLastTurnIndex(thread: Thread): number {
  let max = -1
  for (const turn of thread.turns) {
    const index = Number(turn.id)
    if (!Number.isNaN(index) && index > max) max = index
  }
  return max
}

export function useHarnessConversation(
  sessionId: string | undefined,
  model: string,
): HarnessConversation {
  // The client is recreated whenever the session changes; the previous one is
  // closed by the cleanup effect below.
  const client = useMemo(() => new HarnessClient({ sessionId: sessionId ?? "" }), [sessionId])

  const [restoredMessages, setRestoredMessages] = useState<UIMessage[]>([])
  const [restoredThreadId, setRestoredThreadId] = useState<string | null>(null)
  const [activeConversation, setActiveConversation] = useState<string | undefined>()
  const [restoreVersion, setRestoreVersion] = useState(0)
  const [isHistoryLoading, setIsHistoryLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const transport = useMemo(() => new HarnessChatTransport({ client, model }), [client, model])

  // Close the client when it is replaced or on unmount.
  useEffect(() => {
    return () => {
      client.close()
    }
  }, [client])

  // (Re)restore whenever the session or its client changes.
  useEffect(() => {
    if (!sessionId) {
      client.currentThreadId = null
      client.lastTurnIndex = -1
      setRestoredMessages([])
      setRestoredThreadId(null)
      setActiveConversation(undefined)
      setError(null)
      setIsHistoryLoading(false)
      setRestoreVersion((value) => value + 1)
      return
    }

    let cancelled = false
    setIsHistoryLoading(true)
    setError(null)

    const restore = async () => {
      try {
        await client.open()
        try {
          const { thread } = await client.threadResume({})
          const messages = projectThread(thread)
          client.currentThreadId = thread.id
          client.lastTurnIndex = computeLastTurnIndex(thread)
          if (cancelled) return
          setRestoredMessages(messages)
          setRestoredThreadId(thread.id)
          setActiveConversation(sessionId)
        } catch (resumeError) {
          // A fresh session has no thread to resume — start empty and let the
          // first turn lazily create the thread.
          const message =
            resumeError instanceof Error ? resumeError.message : String(resumeError)
          if (!/no thread to resume/i.test(message)) throw resumeError
          client.currentThreadId = null
          client.lastTurnIndex = -1
          if (cancelled) return
          setRestoredMessages([])
          setRestoredThreadId(null)
          setActiveConversation(sessionId)
        }
      } catch (restoreError) {
        if (cancelled) return
        setError(
          restoreError instanceof Error
            ? restoreError.message
            : "failed to restore conversation",
        )
      } finally {
        if (!cancelled) {
          setIsHistoryLoading(false)
          setRestoreVersion((value) => value + 1)
        }
      }
    }

    void restore()
    return () => {
      cancelled = true
    }
  }, [client, sessionId])

  return {
    transport,
    restoredMessages,
    restoredThreadId,
    activeConversation,
    restoreVersion,
    isHistoryLoading,
    error,
  }
}
