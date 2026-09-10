/**
 * React binding for the core {@link ConversationController}.
 *
 * The conversation state machine (session restore, approvals, compaction,
 * plan mode, …) lives in `@slab/core/harness`; this hook only:
 *   - creates one controller per slab session (`useMemo` keyed on sessionId —
 *     a session change means a pristine controller),
 *   - starts/disposes it with the mount effect,
 *   - exposes the store via `useSyncExternalStore`, and
 *   - builds the AI-SDK {@link HarnessChatTransport} on the controller's
 *     client (switching the model rebuilds only the transport, never the
 *     conversation state).
 */

import { useEffect, useMemo, useSyncExternalStore } from "react"
import type { UIMessage } from "ai"

import {
  ConversationController,
  HarnessChatTransport,
  type ConversationState,
} from "@slab/core/harness"
import type { ApprovalScope, PermissionMode } from "@slab/api/harness"

export interface HarnessConversation extends ConversationState {
  /** Transport bound to the live client (always defined; safe to pass to `useChat`). */
  transport: HarnessChatTransport<UIMessage>
  /** Toggle plan mode on/off. `/plan` and the plan chip use this. */
  setPlanMode: (enabled: boolean) => void
  /** Record a mid-conversation model switch as an in-stream settings marker. */
  noteModelSwitch: (change: { from: string; to: string }, afterMessageId?: string | null) => void
  /** Record a composer permission-mode change as an in-stream settings marker. */
  notePermissionModeChange: (
    change: { from: PermissionMode; to: PermissionMode },
    afterMessageId?: string | null,
  ) => void
  /**
   * Record an approval-review config save (reviewer model / policy prompt) as
   * an in-stream settings marker.
   */
  noteApprovalReviewChange: (
    change: {
      fromModel: string | null
      toModel: string | null
      promptChanged: boolean
    },
    afterMessageId?: string | null,
  ) => void
  /** Resolve a pending approval via `approval/resolve` with a persistence scope. */
  resolveApproval: (itemId: string, approved: boolean, scope: ApprovalScope) => Promise<void>
  /** Manually compact the current (or given) thread via `thread/compact/start`. */
  compactThread: (threadId?: string) => Promise<void>
  /** Fork the current (or given) thread via `thread/fork`, then switch to the child. */
  forkThread: (threadId?: string) => Promise<void>
  /** Retract `turnIndex` and every later turn via `thread/rollback` (turn 0 is a no-op). */
  rollbackFromTurn: (turnIndex: number) => Promise<void>
  /** Steering send — queue user input on the RUNNING turn (iteration boundary). */
  sendSteering: (message: UIMessage, options?: Parameters<ConversationController["send"]>[1]) => Promise<unknown>
  /** Interrupt the live turn (Stop control). */
  interrupt: () => Promise<void>
  /**
   * The pane unmounted while its `useChat` stream was attached — flags the
   * terminal-triggered resync so the orphaned run's reply remounts with the
   * pane instead of staying invisible until a manual reload.
   */
  notifyPaneDetachedMidRun: () => void
}

export function useHarnessConversation(
  sessionId: string | undefined,
  model: string,
): HarnessConversation {
  // One controller per session: a session change constructs a fresh controller
  // (pristine state); the previous one is disposed by the cleanup below.
  const controller = useMemo(() => new ConversationController({ sessionId }), [sessionId])

  // Keep the controller's programmatic-send model in sync with the selected
  // model (steering + non-transport sends) — otherwise they fall back to the
  // controller's fabricated default id, which the server rejects, losing the
  // input. Mirrors the transport memo below: switching the model must never
  // rebuild the conversation state.
  useEffect(() => {
    controller.setModel(model)
  }, [controller, model])

  useEffect(() => {
    controller.start()
    return () => {
      controller.dispose()
    }
  }, [controller])

  const state = useSyncExternalStore(
    controller.subscribe,
    controller.getState,
    controller.getState,
  )

  const transport = useMemo(
    () =>
      new HarnessChatTransport({
        client: controller.client,
        model,
        // Local-stream lifecycle → controller: the mid-run restoreVersion
        // suppression (begin/end) and the accepted-turn ack the pane's
        // failed-draft re-stage keys off (turn started).
        onLocalStreamBegin: controller.markLocalStreamBegin,
        onLocalTurnStarted: () => controller.markLocalTurnStarted(),
        onLocalStreamEnd: controller.markLocalStreamEnd,
      }),
    [controller, model],
  )

  return {
    transport,
    ...state,
    // Arrow-bound on the controller, so these are detachable stable references.
    setPlanMode: controller.setPlanMode,
    noteModelSwitch: controller.noteModelSwitch,
    notePermissionModeChange: controller.notePermissionModeChange,
    noteApprovalReviewChange: controller.noteApprovalReviewChange,
    resolveApproval: controller.resolveApproval,
    compactThread: controller.compactThread,
    forkThread: controller.forkThread,
    rollbackFromTurn: controller.rollbackFromTurn,
    sendSteering: controller.sendSteering,
    interrupt: controller.interrupt,
    notifyPaneDetachedMidRun: controller.notifyPaneDetachedMidRun,
  }
}
