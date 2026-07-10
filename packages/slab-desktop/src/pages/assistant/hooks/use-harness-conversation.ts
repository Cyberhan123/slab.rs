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

import { useCallback, useEffect, useMemo, useState } from "react"
import type { UIMessage } from "ai"

import {
  HARNESS_NOTIFICATION,
  HarnessChatTransport,
  HarnessClient,
  turnItemsToMessages,
  type CommandExecutionRequestApprovalParams,
  type FileChangeApprovalChange,
  type FileChangeRequestApprovalParams,
  type JsonRpcNotification,
  type Thread,
} from "../lib/harness"

/** A pending human-approval request surfaced from the harness (commands / file changes). */
export type ApprovalRequest = {
  itemId: string
  threadId: string
  kind: "command" | "fileChange"
  command?: string
  cwd?: string
  changes?: FileChangeApprovalChange[]
  reason?: string
  status: "pending" | "approved" | "denied"
}

export type ApprovalStatus = "pending" | "approved" | "denied"

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
  /** Approval requests still awaiting a user decision (rendered in the banner). */
  approvals: ApprovalRequest[]
  /** itemId → approval status, for the in-stream tool-card status badge. */
  approvalStatusByItemId: ReadonlyMap<string, ApprovalStatus>
  /** Resolve a pending approval via `approval/resolve`. */
  resolveApproval: (itemId: string, approved: boolean) => Promise<void>
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
  const [approvalMap, setApprovalMap] = useState<Map<string, ApprovalRequest>>(new Map())

  const transport = useMemo(() => new HarnessChatTransport({ client, model }), [client, model])

  /** Track pending human-approval requests independently of the live-turn stream. */
  useEffect(() => {
    return client.onNotification((notification: JsonRpcNotification) => {
      const { method } = notification
      const isCommandApproval = method === HARNESS_NOTIFICATION.ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL
      const isFileApproval = method === HARNESS_NOTIFICATION.ITEM_FILE_CHANGE_REQUEST_APPROVAL
      if (!isCommandApproval && !isFileApproval) return

      const params = (notification.params ?? {}) as
        | CommandExecutionRequestApprovalParams
        | FileChangeRequestApprovalParams
      // Only track approvals for the currently bound thread on this socket.
      if (params.threadId !== client.currentThreadId) return

      setApprovalMap((prev) => {
        if (prev.has(params.itemId)) return prev
        const next = new Map(prev)
        if (isCommandApproval) {
          const command = params as CommandExecutionRequestApprovalParams
          next.set(params.itemId, {
            itemId: params.itemId,
            threadId: params.threadId,
            kind: "command",
            command: command.command,
            cwd: command.cwd,
            reason: command.reason,
            status: "pending",
          })
        } else {
          const file = params as FileChangeRequestApprovalParams
          next.set(params.itemId, {
            itemId: params.itemId,
            threadId: params.threadId,
            kind: "fileChange",
            changes: file.changes,
            status: "pending",
          })
        }
        return next
      })
    })
  }, [client])

  const resolveApproval = useCallback(
    async (itemId: string, approved: boolean) => {
      const entry = approvalMap.get(itemId)
      if (!entry) return
      // Optimistically mark resolved so the banner/card update immediately.
      setApprovalMap((prev) => {
        const existing = prev.get(itemId)
        if (!existing) return prev
        const next = new Map(prev)
        next.set(itemId, { ...existing, status: approved ? "approved" : "denied" })
        return next
      })
      try {
        const result = await client.approvalResolve({ threadId: entry.threadId, itemId, approved })
        // If the server couldn't deliver the decision (e.g. the pending entry
        // was gone), revert to pending so the user sees it wasn't actioned.
        if (result.delivered === false) {
          setApprovalMap((prev) => {
            const existing = prev.get(itemId)
            if (!existing) return prev
            const next = new Map(prev)
            next.set(itemId, { ...existing, status: "pending" })
            return next
          })
          throw new Error("approval not delivered")
        }
      } catch (resolveError) {
        // Revert to pending if the server rejected the resolution.
        setApprovalMap((prev) => {
          const existing = prev.get(itemId)
          if (!existing) return prev
          const next = new Map(prev)
          next.set(itemId, { ...existing, status: "pending" })
          return next
        })
        throw resolveError
      }
    },
    [approvalMap, client],
  )

  const approvals = useMemo(
    () => Array.from(approvalMap.values()).filter((a) => a.status === "pending"),
    [approvalMap],
  )
  const approvalStatusByItemId = useMemo(() => {
    const map = new Map<string, ApprovalStatus>()
    for (const [id, req] of approvalMap) map.set(id, req.status)
    return map
  }, [approvalMap])

  // Close the client when it is replaced or on unmount.
  useEffect(() => {
    return () => {
      client.close()
    }
  }, [client])

  // (Re)restore whenever the session or its client changes.
  useEffect(() => {
    // A new session means a new thread; drop any stale approval state.
    setApprovalMap(new Map())
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
          const messages = turnItemsToMessages(thread.turns.flatMap((turn) => turn.items))
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
    approvals,
    approvalStatusByItemId,
    resolveApproval,
  }
}
