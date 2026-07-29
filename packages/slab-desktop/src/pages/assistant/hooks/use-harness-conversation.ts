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
  type ApprovalScope,
  type CommandExecutionOutputDeltaParams,
  type FileChangeOutputDeltaParams,
  type CommandExecutionRequestApprovalParams,
  type ContextCompactedParams,
  type ContextCompactingParams,
  type FileChangeApprovalChange,
  type FileChangeRequestApprovalParams,
  type JsonRpcNotification,
  type ModelLoadDeltaParams,
  type ModelLoadPhase,
  type OperationCategory,
  type Thread,
  type TurnCompletedParams,
  type TurnUsage,
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
  category?: OperationCategory
  /** Persistence scopes the server allows the user to pick. */
  allowedScopes?: ApprovalScope[]
  status: "pending" | "approved" | "denied"
}

export type ApprovalStatus = "pending" | "approved" | "denied"

/**
 * Transient model-load indicator state, driven by `model/load/delta` +
 * `model/load/completed` notifications emitted from `turn/start`. `null` when
 * no load is in progress (the indicator is hidden).
 */
export type ModelLoadState = {
  phase: ModelLoadPhase
  modelId?: string
  downloadedBytes?: number
  totalBytes?: number
} | null

/** Whether a compaction marker represents an automatic or manual (`/compact`) run. */
export type CompactionMode = "auto" | "manual"
/** `compacting` = in-progress (rendered as a Shimmer); `compacted` = done. */
export type CompactionPhase = "compacting" | "compacted"

/**
 * A session-scoped compaction divider rendered in the message stream. Survives
 * the pane remount after a manual compact (state lives in this hook, above the
 * keyed pane) but is cleared on a full reload — not persisted to the backend.
 */
export interface CompactionMarker {
  /** Stable id (`${mode}:${threadId}:${nonce}`) so phase flips keep the same row. */
  id: string
  mode: CompactionMode
  phase: CompactionPhase
  threadId: string
}

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
  /** itemId → accumulated live command output (stdout/stderr deltas so far). */
  liveOutputByItemId: ReadonlyMap<string, string>
  /** itemId → live `apply_patch` progress lines (one JSON object per applied file). */
  livePatchByItemId: ReadonlyMap<string, string[]>
  /** Transient model-load indicator state (null when idle). */
  modelLoad: ModelLoadState
  /** Token usage for the most recent completed turn (null until the first turn completes). */
  turnUsage: TurnUsage | null
  /** `thread.createdAt` (Unix ms) of the restored thread; null for fresh sessions. */
  historyCreatedAt: number | null
  /** Session-scoped compaction markers rendered as in-stream dividers. */
  compactionMarkers: CompactionMarker[]
  /** True while a manual `/compact` round-trip is in flight. */
  isCompacting: boolean
  /** Resolve a pending approval via `approval/resolve` with a persistence scope. */
  resolveApproval: (itemId: string, approved: boolean, scope: ApprovalScope) => Promise<void>
  /** Manually compact the current (or given) thread via `thread/compact/start`. */
  compactThread: (threadId?: string) => Promise<void>
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
  const [liveOutputMap, setLiveOutputMap] = useState<Map<string, string>>(new Map())
  const [livePatchMap, setLivePatchMap] = useState<Map<string, string[]>>(new Map())
  const [modelLoad, setModelLoad] = useState<ModelLoadState>(null)
  const [turnUsage, setTurnUsage] = useState<TurnUsage | null>(null)
  const [historyCreatedAt, setHistoryCreatedAt] = useState<number | null>(null)
  const [compactionMarkers, setCompactionMarkers] = useState<CompactionMarker[]>([])
  const [isCompacting, setIsCompacting] = useState(false)

  const transport = useMemo(() => new HarnessChatTransport({ client, model }), [client, model])

  /** Track pending human-approval requests independently of the live-turn stream. */
  useEffect(() => {
    return client.onNotification((notification: JsonRpcNotification) => {
      const { method } = notification

      // Accumulate live command output (stdout/stderr deltas) so the terminal
      // card can render output as it streams, before `item/completed` finalizes.
      if (method === HARNESS_NOTIFICATION.ITEM_COMMAND_EXECUTION_OUTPUT_DELTA) {
        const params = (notification.params ?? {}) as CommandExecutionOutputDeltaParams
        if (params.threadId !== client.currentThreadId) return
        setLiveOutputMap((prev) => {
          const existing = prev.get(params.itemId) ?? ""
          // Bound per-item accumulation so a runaway command can't exhaust memory.
          if (existing.length + params.delta.length > 256 * 1024) return prev
          const next = new Map(prev)
          next.set(params.itemId, existing + params.delta)
          return next
        })
        return
      }

      // Accumulate live apply_patch progress (one JSON line per committed file)
      // so the file-change card can show files as they apply, before
      // `item/completed` finalizes the change set.
      if (method === HARNESS_NOTIFICATION.ITEM_FILE_CHANGE_OUTPUT_DELTA) {
        const params = (notification.params ?? {}) as FileChangeOutputDeltaParams
        if (params.threadId !== client.currentThreadId) return
        setLivePatchMap((prev) => {
          const existing = prev.get(params.itemId) ?? []
          // Bound per-item accumulation so a huge patch can't exhaust memory.
          if (existing.length >= 1024) return prev
          const next = new Map(prev)
          next.set(params.itemId, [...existing, params.delta])
          return next
        })
        return
      }

      // Transient model-load indicator: set on each delta, clear on completed.
      // The load is per-turn and short-lived; a `completed` always resets it, so
      // no thread filtering is needed (one active turn on the socket at a time).
      if (method === HARNESS_NOTIFICATION.MODEL_LOAD_DELTA) {
        const params = (notification.params ?? {}) as ModelLoadDeltaParams
        setModelLoad({
          phase: params.phase,
          modelId: params.modelId,
          downloadedBytes: params.downloadedBytes,
          totalBytes: params.totalBytes,
        })
        return
      }
      if (method === HARNESS_NOTIFICATION.MODEL_LOAD_COMPLETED) {
        setModelLoad(null)
        return
      }

      // Context-compaction lifecycle: an in-progress auto-compaction adds a
      // "compacting" marker; the terminal event either flips it to "compacted"
      // or removes it (status "skipped" = a started compaction that didn't shrink).
      if (method === HARNESS_NOTIFICATION.CONTEXT_COMPACTING) {
        const params = (notification.params ?? {}) as ContextCompactingParams
        if (params.threadId !== client.currentThreadId) return
        setCompactionMarkers((prev) => {
          // Only one auto-compacting marker per thread at a time.
          if (
            prev.some(
              (m) =>
                m.mode === "auto" &&
                m.phase === "compacting" &&
                m.threadId === params.threadId,
            )
          ) {
            return prev
          }
          return [
            ...prev,
            {
              id: `auto:${params.threadId}:${Date.now()}`,
              mode: "auto",
              phase: "compacting",
              threadId: params.threadId,
            },
          ]
        })
        return
      }
      if (method === HARNESS_NOTIFICATION.CONTEXT_COMPACTED) {
        const params = (notification.params ?? {}) as ContextCompactedParams
        if (params.threadId !== client.currentThreadId) return
        setCompactionMarkers((prev) => {
          if (params.status === "skipped") {
            // Started but did not compact — drop the in-progress marker.
            return prev.filter(
              (m) =>
                !(m.mode === "auto" && m.threadId === params.threadId && m.phase === "compacting"),
            )
          }
          return prev.map((m) =>
            m.mode === "auto" && m.threadId === params.threadId && m.phase === "compacting"
              ? { ...m, phase: "compacted" }
              : m,
          )
        })
        return
      }

      // Capture the finalized token usage for the just-completed turn so the
      // composer footer can render a usage indicator + context-window bar.
      if (method === HARNESS_NOTIFICATION.TURN_COMPLETED) {
        const params = (notification.params ?? {}) as TurnCompletedParams
        setTurnUsage(params.usage ?? null)
        return
      }

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
            category: command.category,
            allowedScopes: command.allowedScopes,
            status: "pending",
          })
        } else {
          const file = params as FileChangeRequestApprovalParams
          next.set(params.itemId, {
            itemId: params.itemId,
            threadId: params.threadId,
            kind: "fileChange",
            changes: file.changes,
            allowedScopes: file.allowedScopes,
            status: "pending",
          })
        }
        return next
      })
    })
  }, [client])

  const resolveApproval = useCallback(
    async (itemId: string, approved: boolean, scope: ApprovalScope) => {
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
        const result = await client.approvalResolve({ threadId: entry.threadId, itemId, approved, scope })
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

  const compactThread = useCallback(
    async (threadId?: string) => {
      const tid = threadId ?? client.currentThreadId
      if (!tid) return
      setError(null)
      setIsCompacting(true)
      const markerId = `manual:${tid}:${Date.now()}`
      setCompactionMarkers((prev) => [
        ...prev,
        { id: markerId, mode: "manual", phase: "compacting", threadId: tid },
      ])
      try {
        await client.threadCompactStart({ threadId: tid })
        // Re-resume so the pane re-renders with the compacted history.
        const { thread } = await client.threadResume({ threadId: tid })
        const messages = turnItemsToMessages(thread.turns.flatMap((turn) => turn.items))
        client.lastTurnIndex = computeLastTurnIndex(thread)
        setRestoredMessages(messages)
        // historyCreatedAt is unchanged (same thread). Flip the marker to done;
        // it survives the restoreVersion remount because it lives in this hook.
        setCompactionMarkers((prev) =>
          prev.map((m) => (m.id === markerId ? { ...m, phase: "compacted" } : m)),
        )
        setRestoreVersion((value) => value + 1)
      } catch (compactError) {
        // Drop the marker — no compaction happened.
        setCompactionMarkers((prev) => prev.filter((m) => m.id !== markerId))
        setError(compactError instanceof Error ? compactError.message : "compact failed")
      } finally {
        setIsCompacting(false)
      }
    },
    [client],
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
    setLiveOutputMap(new Map())
    setLivePatchMap(new Map())
    setModelLoad(null)
    setTurnUsage(null)
    setHistoryCreatedAt(null)
    setCompactionMarkers([])
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
          setHistoryCreatedAt(thread.createdAt)
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
    liveOutputByItemId: liveOutputMap,
    livePatchByItemId: livePatchMap,
    modelLoad,
    turnUsage,
    historyCreatedAt,
    compactionMarkers,
    isCompacting,
    resolveApproval,
    compactThread,
  }
}
