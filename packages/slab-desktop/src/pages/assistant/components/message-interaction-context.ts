/**
 * Interaction context for the message tree. Currently carries the per-item
 * human-approval status so the in-stream tool card can render an accurate
 * "Awaiting Approval"/"Denied" badge, plus the live (in-flight) command output
 * accumulated from `item/commandExecution/outputDelta` so the terminal card can
 * render output as it streams (before the finalized `item/completed` arrives).
 * The approve/reject *actions* live in the separate approval banner
 * (`components/approval-banner.tsx`); this context is display-only.
 */

import { createContext, useContext } from "react"

export type ApprovalStatus = "pending" | "approved" | "denied"

export interface MessageInteractionValue {
  /** itemId → approval status, sourced from the harness approval notifications. */
  approvalStatusByItemId: ReadonlyMap<string, ApprovalStatus>
  /** itemId → accumulated live command output (stdout/stderr deltas so far). */
  liveOutputByItemId: ReadonlyMap<string, string>
}

export const MessageInteractionContext = createContext<MessageInteractionValue>({
  approvalStatusByItemId: new Map<string, ApprovalStatus>(),
  liveOutputByItemId: new Map<string, string>(),
})

export const useMessageInteraction = (): MessageInteractionValue =>
  useContext(MessageInteractionContext)
