/**
 * Interaction context for the message tree. Currently carries the per-item
 * human-approval status so the in-stream tool card can render an accurate
 * "Awaiting Approval"/"Denied" badge, plus the live (in-flight) command output
 * accumulated from `item/commandExecution/outputDelta` so the terminal card can
 * render output as it streams (before the finalized `item/completed` arrives).
 * The approve/reject *actions* live in the separate approval banner
 * (`components/approval-banner.tsx`).
 *
 * It also carries the per-user-message turn index (so a user bubble knows its
 * turn for the rollback affordance) and the rollback action itself.
 */

import { createContext, useContext } from "react"

export type ApprovalStatus = "pending" | "approved" | "denied"

export interface MessageInteractionValue {
  /** itemId → approval status, sourced from the harness approval notifications. */
  approvalStatusByItemId: ReadonlyMap<string, ApprovalStatus>
  /** itemId → accumulated live command output (stdout/stderr deltas so far). */
  liveOutputByItemId: ReadonlyMap<string, string>
  /** itemId → live `apply_patch` progress lines (one JSON object per applied file). */
  livePatchByItemId: ReadonlyMap<string, string[]>
  /** userMessage itemId → its numeric turn index (drives the rollback affordance). */
  userMessageTurnIndex: ReadonlyMap<string, number>
  /** Retract the user message with this id and everything after it. */
  rollbackToMessage: ((messageId: string) => void) | undefined
}

export const MessageInteractionContext = createContext<MessageInteractionValue>({
  approvalStatusByItemId: new Map<string, ApprovalStatus>(),
  liveOutputByItemId: new Map<string, string>(),
  livePatchByItemId: new Map<string, string[]>(),
  userMessageTurnIndex: new Map<string, number>(),
  rollbackToMessage: undefined,
})

export const useMessageInteraction = (): MessageInteractionValue =>
  useContext(MessageInteractionContext)
