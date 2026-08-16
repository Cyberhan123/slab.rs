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
  type ApprovalScope,
  type ConversationState,
} from "@slab/core/harness"

export interface HarnessConversation extends ConversationState {
  /** Transport bound to the live client (always defined; safe to pass to `useChat`). */
  transport: HarnessChatTransport<UIMessage>
  /** Toggle plan mode on/off. `/plan` and the plan chip use this. */
  setPlanMode: (enabled: boolean) => void
  /** Resolve a pending approval via `approval/resolve` with a persistence scope. */
  resolveApproval: (itemId: string, approved: boolean, scope: ApprovalScope) => Promise<void>
  /** Manually compact the current (or given) thread via `thread/compact/start`. */
  compactThread: (threadId?: string) => Promise<void>
  /** Fork the current (or given) thread via `thread/fork`, then switch to the child. */
  forkThread: (threadId?: string) => Promise<void>
  /** Retract `turnIndex` and every later turn via `thread/rollback` (turn 0 is a no-op). */
  rollbackFromTurn: (turnIndex: number) => Promise<void>
}

export function useHarnessConversation(
  sessionId: string | undefined,
  model: string,
): HarnessConversation {
  // One controller per session: a session change constructs a fresh controller
  // (pristine state); the previous one is disposed by the cleanup below.
  const controller = useMemo(() => new ConversationController({ sessionId }), [sessionId])

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
    () => new HarnessChatTransport({ client: controller.client, model }),
    [controller, model],
  )

  return {
    transport,
    ...state,
    // Arrow-bound on the controller, so these are detachable stable references.
    setPlanMode: controller.setPlanMode,
    resolveApproval: controller.resolveApproval,
    compactThread: controller.compactThread,
    forkThread: controller.forkThread,
    rollbackFromTurn: controller.rollbackFromTurn,
  }
}
